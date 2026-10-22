// Included beside BoringSSL's existing key-share classes. No new cryptographic primitives.
class GatewayMLKEM768KeyShare final : public SSLKeyShare {
 public:
  ~GatewayMLKEM768KeyShare() override {
    OPENSSL_cleanse(&private_key_, sizeof(private_key_));
  }
  uint16_t GroupID() const override { return 0; }  // Internal component, never advertised.
  bool Generate(CBB *out) override {
    uint8_t public_key[MLKEM768_PUBLIC_KEY_BYTES];
    MLKEM768_generate_key(public_key, nullptr, &private_key_);
    return CBB_add_bytes(out, public_key, sizeof(public_key));
  }
  bool Encap(CBB *out, Array<uint8_t> *secret, uint8_t *alert,
             Span<const uint8_t> peer) override {
    *alert = SSL_AD_INTERNAL_ERROR;
    CBS input;
    CBS_init(&input, peer.data(), peer.size());
    MLKEM768_public_key public_key;
    if (!MLKEM768_parse_public_key(&public_key, &input) || CBS_len(&input) != 0) {
      *alert = SSL_AD_ILLEGAL_PARAMETER;
      return false;
    }
    Array<uint8_t> result;
    if (!result.InitForOverwrite(MLKEM_SHARED_SECRET_BYTES)) { return false; }
    uint8_t ciphertext[MLKEM768_CIPHERTEXT_BYTES];
    MLKEM768_encap(ciphertext, result.data(), &public_key);
    if (!CBB_add_bytes(out, ciphertext, sizeof(ciphertext))) { return false; }
    *secret = std::move(result);
    return true;
  }
  bool Decap(Array<uint8_t> *secret, uint8_t *alert,
             Span<const uint8_t> ciphertext) override {
    *alert = SSL_AD_INTERNAL_ERROR;
    Array<uint8_t> result;
    if (!result.InitForOverwrite(MLKEM_SHARED_SECRET_BYTES)) { return false; }
    // Keep ML-KEM implicit rejection; do not create a ciphertext-validity oracle.
    if (!MLKEM768_decap(result.data(), ciphertext.data(), ciphertext.size(), &private_key_)) {
      *alert = SSL_AD_ILLEGAL_PARAMETER;
      return false;
    }
    *secret = std::move(result);
    return true;
  }
 private:
  MLKEM768_private_key private_key_;
};

class GatewayECHybridKeyShare final : public SSLKeyShare {
 public:
  explicit GatewayECHybridKeyShare(bool p384)
      : group_(p384 ? SSL_GROUP_SECP384R1_MLKEM1024 : SSL_GROUP_SECP256R1_MLKEM768),
        ec_bytes_(p384 ? 97 : 65),
        public_bytes_(p384 ? MLKEM1024_PUBLIC_KEY_BYTES : MLKEM768_PUBLIC_KEY_BYTES),
        ciphertext_bytes_(p384 ? MLKEM1024_CIPHERTEXT_BYTES : MLKEM768_CIPHERTEXT_BYTES),
        ec_(MakeUnique<ECKeyShare>(p384 ? EC_group_p384() : EC_group_p256(),
                                  p384 ? SSL_GROUP_SECP384R1 : SSL_GROUP_SECP256R1)) {
    if (p384) { kem_ = MakeUnique<MLKEM1024KeyShare>(); }
    else { kem_ = MakeUnique<GatewayMLKEM768KeyShare>(); }
  }
  uint16_t GroupID() const override { return group_; }
  bool Generate(CBB *out) override {
    return ec_ && kem_ && ec_->Generate(out) && kem_->Generate(out);
  }
  bool Encap(CBB *out, Array<uint8_t> *secret, uint8_t *alert,
             Span<const uint8_t> peer) override {
    *alert = SSL_AD_INTERNAL_ERROR;
    if (peer.size() != ec_bytes_ + public_bytes_) {
      *alert = SSL_AD_ILLEGAL_PARAMETER;
      return false;
    }
    Array<uint8_t> ec_secret, kem_secret;
    return ec_ && kem_ && ec_->Encap(out, &ec_secret, alert, peer.first(ec_bytes_)) &&
           kem_->Encap(out, &kem_secret, alert, peer.subspan(ec_bytes_)) &&
           Combine(secret, ec_secret, kem_secret);
  }
  bool Decap(Array<uint8_t> *secret, uint8_t *alert,
             Span<const uint8_t> peer) override {
    *alert = SSL_AD_INTERNAL_ERROR;
    if (peer.size() != ec_bytes_ + ciphertext_bytes_) {
      *alert = SSL_AD_ILLEGAL_PARAMETER;
      return false;
    }
    Array<uint8_t> ec_secret, kem_secret;
    return ec_ && kem_ && ec_->Decap(&ec_secret, alert, peer.first(ec_bytes_)) &&
           kem_->Decap(&kem_secret, alert, peer.subspan(ec_bytes_)) &&
           Combine(secret, ec_secret, kem_secret);
  }
 private:
  static bool Combine(Array<uint8_t> *out, Span<const uint8_t> ec,
                      Span<const uint8_t> kem) {
    Array<uint8_t> result;
    if (!result.InitForOverwrite(ec.size() + kem.size())) { return false; }
    // These two groups use EC || ML-KEM, unlike X25519MLKEM768.
    OPENSSL_memcpy(result.data(), ec.data(), ec.size());
    OPENSSL_memcpy(result.data() + ec.size(), kem.data(), kem.size());
    *out = std::move(result);
    return true;
  }
  const uint16_t group_;
  const size_t ec_bytes_, public_bytes_, ciphertext_bytes_;
  UniquePtr<SSLKeyShare> ec_, kem_;
};
