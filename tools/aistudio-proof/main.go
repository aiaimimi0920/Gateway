// Isolated process wrapper for the MIT-licensed AIStudio2API WAA runtime.
// stdin/stdout are private parent-child pipes. Never log requests, cookies or proof values.
package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"os"
	"strings"
	"time"

	"github.com/Mag1cFall/AIStudio2API/internal/waa"
)

type input struct {
	Challenge   json.RawMessage `json:"challenge"`
	Interpreter string          `json:"interpreter"`
	Profile     waa.Profile     `json:"profile"`
	Prompt      string          `json:"prompt"`
}

type output struct {
	OK         bool   `json:"ok"`
	Proof      string `json:"proof,omitempty"`
	Error      string `json:"error,omitempty"`
	ErrorClass string `json:"errorClass,omitempty"`
	InitMS     int64  `json:"initMs"`
	ProofMS    int64  `json:"proofMs"`
}

func main() {
	out := run()
	_ = json.NewEncoder(os.Stdout).Encode(out)
	if !out.OK {
		os.Exit(1)
	}
}

func errorOutput(stage string, err error) output {
	class := "runtime_error"
	for _, label := range []string{"TypeError", "ReferenceError", "SyntaxError", "InternalError", "RangeError"} {
		if strings.Contains(err.Error(), label) {
			class = label
			break
		}
	}
	if errors.Is(err, context.DeadlineExceeded) {
		class = "timeout"
	}
	return output{Error: stage, ErrorClass: class}
}

func run() output {
	data, err := io.ReadAll(io.LimitReader(os.Stdin, (12<<20)+1))
	if err != nil || len(data) > 12<<20 {
		return output{Error: "input_limit"}
	}
	var request input
	if err = json.Unmarshal(data, &request); err != nil {
		return output{Error: "input_invalid"}
	}
	if len(request.Interpreter) > 2<<20 || len(request.Prompt) > 9<<20 || request.Prompt == "" {
		return output{Error: "input_invalid"}
	}
	challenge, err := waa.ParseChallenge(request.Challenge)
	if err != nil {
		return errorOutput("challenge_invalid", err)
	}
	if waa.InterpreterHash([]byte(request.Interpreter)) != challenge.InterpreterHash {
		return output{Error: "interpreter_digest_mismatch"}
	}
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	started := time.Now()
	runtime, err := waa.NewRuntime(ctx, waa.Options{Challenge: challenge, Interpreter: request.Interpreter,
		Profile: request.Profile, LoadImage: loadPublicImage})
	if err != nil {
		return errorOutput("runtime_init_failed", err)
	}
	defer runtime.Close()
	if err = runtime.FillPrompt(ctx, "Hello", true); err != nil {
		return errorOutput("warmup_failed", err)
	}
	if err = runtime.Settle(ctx); err != nil {
		return errorOutput("settle_failed", err)
	}
	out := output{InitMS: time.Since(started).Milliseconds()}
	digest := sha256.Sum256([]byte(request.Prompt))
	started = time.Now()
	proof, err := runtime.Proof(ctx, hex.EncodeToString(digest[:]), request.Prompt)
	if err != nil {
		return errorOutput("proof_failed", err)
	}
	if !strings.HasPrefix(proof, "!") || len(proof) > 64<<10 {
		return output{Error: "proof_invalid"}
	}
	out.OK, out.Proof, out.ProofMS = true, proof, time.Since(started).Milliseconds()
	return out
}
