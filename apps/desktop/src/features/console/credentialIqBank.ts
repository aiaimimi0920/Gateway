import type { CredentialTestCase } from "../../api/contracts";

// Original deterministic exercises, not copied from an unavailable external question bank.
export const IQ_BANK_VERSION = "neuro-reasoning-v1";
export const iqExtraCases: CredentialTestCase[] = [
  { id: "iq-sequence-v1", name: "数列递推", difficulty: 1, prompt: "Start with 3. Repeatedly multiply by 2 and add 1. After 5 repetitions, what is the value? Reply with only the integer.", expectedAnswer: "127", enabled: false },
  { id: "iq-count-v1", name: "容斥计数", difficulty: 2, prompt: "How many integers from 1 to 120 inclusive are divisible by 4 or by 6, but not by both? Reply with only the integer.", expectedAnswer: "30", enabled: false },
  { id: "iq-bayes-v1", name: "条件概率", difficulty: 3, prompt: "Box A has 3 red and 1 blue ball. Box B has 1 red and 3 blue balls. Select one box uniformly at random and draw one ball. It is red. What is the probability the selected box was A? Reply with only a reduced fraction a/b.", expectedAnswer: "3/4", enabled: false },
  { id: "iq-order-v1", name: "偏序约束", difficulty: 2, prompt: "Four tasks A, B, C, D each run once in a sequence. A must precede C; B must precede C; C must precede D. How many valid sequences are there? Reply with only the integer.", expectedAnswer: "2", enabled: false },
  { id: "iq-path-v1", name: "网格路径", difficulty: 3, prompt: "On a grid, walk from (0,0) to (4,3), using only unit right or unit up moves. How many paths avoid (2,1)? Reply with only the integer.", expectedAnswer: "17", enabled: false },
  { id: "iq-state-v1", name: "程序状态跟踪", difficulty: 2, prompt: "Let x=2 and y=1. Repeat 4 times, simultaneously assigning x=x+y and y=the old x. What is x+y after the fourth repetition? Reply with only the integer.", expectedAnswer: "21", enabled: false },
  { id: "iq-modular-v1", name: "同余推理", difficulty: 3, prompt: "Find the smallest positive integer n such that n mod 5=2, n mod 7=3, and n mod 9=4. Reply with only the integer.", expectedAnswer: "157", enabled: false },
  { id: "iq-language-v1", name: "否定条件理解", difficulty: 1, prompt: "A gate opens if and only if card K is present and alarm A is off. K is present, but A is on. Is the gate open? Reply with only YES or NO.", expectedAnswer: "NO", enabled: false },
  { id: "iq-combinatorics-v1", name: "非相邻组合", difficulty: 3, prompt: "How many ways are there to choose 3 distinct numbers from 1 through 8 inclusive so that no two chosen numbers are consecutive? Order does not matter. Reply with only the integer.", expectedAnswer: "20", enabled: false },
];

export function appendIqCases(cases: CredentialTestCase[]): CredentialTestCase[] {
  const ids = new Set(cases.map((item) => item.id));
  return [...cases, ...iqExtraCases.filter((item) => !ids.has(item.id)).slice(0, Math.max(0, 16 - cases.length)).map((item) => ({ ...item }))];
}
