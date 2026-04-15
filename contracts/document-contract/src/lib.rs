#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    Address, BytesN, Env, String,
    panic_with_error, log, symbol_short,
};

// ================================================================
// ERROR CODES
// ================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized      = 1, // Caller bukan admin
    DocAlreadyExists   = 2, // Hash dokumen sudah terdaftar
    DocNotFound        = 3, // Hash dokumen tidak ditemukan
    DocumentRevoked    = 4, // Dokumen sudah dicabut
    AlreadyInitialized = 5, // Kontrak sudah di-init sebelumnya
    NotInitialized     = 6, // Kontrak belum di-init
}

// ================================================================
// STORAGE KEYS — typed enum mencegah key collision antar data
// ================================================================

#[contracttype]
pub enum DataKey {
    Admin,                // Alamat admin aktif (instance storage)
    Document(BytesN<32>), // Data dokumen, diindex by SHA-256 hash
    DocCount,             // Total dokumen yang pernah diterbitkan
}

// ================================================================
// DATA STRUCTURES
// ================================================================

#[contracttype]
#[derive(Clone, Debug)]
pub struct Document {
    pub hash:       BytesN<32>,  // SHA-256 dari file asli (off-chain)
    pub issuer:     Address,     // Wallet penerbit dokumen
    pub owner_name: String,      // Nama pemilik dokumen
    pub issued_at:  u64,         // Timestamp penerbitan (Unix epoch)
    pub revoked_at: Option<u64>, // None = aktif | Some(ts) = dicabut pada ts tsb
    pub metadata:   String,      // Info tambahan: nomor seri, jenis dokumen, dll.
}

// Struct hasil verifikasi publik — mencakup status + data + waktu cek
#[contracttype]
#[derive(Clone, Debug)]
pub struct VerifyResult {
    pub document:   Document, // Data lengkap dokumen
    pub is_valid:   bool,     // true jika belum pernah dicabut
    pub checked_at: u64,      // Timestamp saat verifikasi dilakukan
}

// ================================================================
// TTL CONSTANTS
// 1 ledger Stellar ≈ 5 detik → 524_288 ledger ≈ 1 tahun
// ================================================================

const LEDGERS_PER_YEAR: u32   = 524_288;
const TTL_BUMP_THRESHOLD: u32 = LEDGERS_PER_YEAR / 2; // Bump jika sisa TTL < 6 bulan
const TTL_TARGET: u32         = LEDGERS_PER_YEAR;      // Perpanjang hingga 1 tahun penuh

// ================================================================
// CONTRACT
// ================================================================

#[contract]
pub struct DocumentContract;

#[contractimpl]
impl DocumentContract {

    // ----------------------------------------------------------------
    // INTERNAL HELPERS — private, tidak ter-expose sebagai endpoint
    // ----------------------------------------------------------------

    // Ambil alamat admin dari storage, panic jika kontrak belum di-init
    fn load_admin(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    // Pastikan caller adalah admin + minta tanda tangan on-chain
    fn require_admin(env: &Env, caller: &Address) {
        caller.require_auth();
        if *caller != Self::load_admin(env) {
            panic_with_error!(env, Error::NotAuthorized);
        }
    }

    // ----------------------------------------------------------------
    // INIT — dipanggil SEKALI saat deploy, menetapkan admin pertama
    // ----------------------------------------------------------------

    pub fn init(env: Env, admin: Address) {
        // Tolak jika sudah pernah diinisialisasi
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        admin.require_auth(); // Admin harus menandatangani transaksi ini
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::DocCount, &0u64);
        log!(&env, "Contract initialized. Admin: {}", admin);
    }

    // ----------------------------------------------------------------
    // TRANSFER ADMIN — serah terima kepemilikan kontrak secara aman
    // Kedua pihak (lama & baru) wajib menandatangani transaksi
    // ----------------------------------------------------------------

    pub fn transfer_admin(env: Env, current_admin: Address, new_admin: Address) {
        Self::require_admin(&env, &current_admin); // Admin lama konfirmasi
        new_admin.require_auth();                  // Admin baru konfirmasi
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        log!(&env, "Admin transferred to {}", new_admin);

        // Emit event untuk indexer / off-chain listener
        env.events().publish(
            (symbol_short!("adm_xfer"), current_admin),
            new_admin,
        );
    }

    // ----------------------------------------------------------------
    // ISSUE DOCUMENT — terbitkan dokumen baru ke blockchain
    // Hanya admin yang bisa memanggil fungsi ini
    // ----------------------------------------------------------------

    pub fn issue_document(
        env:        Env,
        issuer:     Address,    // Harus sama dengan admin
        doc_hash:   BytesN<32>, // Hash unik dari file dokumen
        owner_name: String,     // Nama pemilik / penerima dokumen
        metadata:   String,     // Nomor seri, jenis, atau info tambahan
    ) {
        Self::require_admin(&env, &issuer);

        let key = DataKey::Document(doc_hash.clone());

        // Tolak jika hash yang sama sudah pernah diterbitkan
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::DocAlreadyExists);
        }

        let doc = Document {
            hash:       doc_hash.clone(),
            issuer,
            owner_name,
            issued_at:  env.ledger().timestamp(),
            revoked_at: None, // Dokumen baru selalu aktif
            metadata,
        };

        env.storage().persistent().set(&key, &doc);
        // Set TTL agar dokumen tidak expire sebelum waktunya
        env.storage().persistent().extend_ttl(&key, TTL_BUMP_THRESHOLD, TTL_TARGET);

        // Increment counter total dokumen
        let count: u64 = env.storage().instance().get(&DataKey::DocCount).unwrap_or(0);
        env.storage().instance().set(&DataKey::DocCount, &(count + 1));

        // Emit event agar sistem off-chain bisa mengindex penerbitan ini
        env.events().publish(
            (symbol_short!("issued"), doc_hash.clone()),
            env.ledger().timestamp(),
        );

        log!(&env, "Document issued. Hash: {}", doc_hash);
    }

    // ----------------------------------------------------------------
    // REVOKE DOCUMENT — cabut dokumen yang sudah diterbitkan
    // Idempotent: aman dipanggil lebih dari sekali pada dokumen yang sama
    // ----------------------------------------------------------------

    pub fn revoke_document(env: Env, admin: Address, doc_hash: BytesN<32>) {
        Self::require_admin(&env, &admin);

        let key = DataKey::Document(doc_hash.clone());

        let mut doc: Document = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::DocNotFound));

        // Jika sudah dicabut sebelumnya, skip tanpa error (idempotent)
        if doc.revoked_at.is_some() {
            return;
        }

        // Catat timestamp pencabutan untuk keperluan audit
        doc.revoked_at = Some(env.ledger().timestamp());
        env.storage().persistent().set(&key, &doc);
        // TTL tidak di-extend — dokumen revoked biarkan expire alami

        // Emit event pencabutan untuk indexer
        env.events().publish(
            (symbol_short!("revoked"), doc_hash.clone()),
            env.ledger().timestamp(),
        );

        log!(&env, "Document revoked. Hash: {}", doc_hash);
    }

    // ----------------------------------------------------------------
    // VERIFY DOCUMENT — verifikasi publik, siapapun bisa memanggil
    // Mengembalikan VerifyResult lengkap (tidak panic jika revoked)
    // ----------------------------------------------------------------

    pub fn verify_document(env: Env, doc_hash: BytesN<32>) -> VerifyResult {
        let key = DataKey::Document(doc_hash.clone());

        let doc: Document = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::DocNotFound));

        // Perpanjang TTL hanya jika dokumen masih aktif
        if doc.revoked_at.is_none() {
            env.storage().persistent().extend_ttl(&key, TTL_BUMP_THRESHOLD, TTL_TARGET);
        }

        VerifyResult {
            is_valid:   doc.revoked_at.is_none(), // false jika sudah dicabut
            checked_at: env.ledger().timestamp(),
            document:   doc,
        }
    }

    // ----------------------------------------------------------------
    // GET DOCUMENT — data mentah tanpa filter status revoke
    // Digunakan untuk keperluan audit internal / admin
    // ----------------------------------------------------------------

    pub fn get_document(env: Env, doc_hash: BytesN<32>) -> Document {
        env.storage()
            .persistent()
            .get(&DataKey::Document(doc_hash))
            .unwrap_or_else(|| panic_with_error!(&env, Error::DocNotFound))
    }

    // ----------------------------------------------------------------
    // VIEW HELPERS — read-only, tidak memerlukan auth
    // ----------------------------------------------------------------

    // Kembalikan alamat admin yang sedang aktif
    pub fn get_admin(env: Env) -> Address {
        Self::load_admin(&env)
    }

    // Total dokumen yang pernah diterbitkan (termasuk yang sudah dicabut)
    pub fn get_doc_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::DocCount).unwrap_or(0)
    }

    // Cek cepat apakah hash terdaftar tanpa fetch data penuh
    pub fn document_exists(env: Env, doc_hash: BytesN<32>) -> bool {
        env.storage().persistent().has(&DataKey::Document(doc_hash))
    }
}

// ================================================================
// UNIT TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, vec, Env, Address, BytesN, String};

    // Helper: buat environment + deploy kontrak + init admin
    fn setup() -> (Env, DocumentContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths(); // Mock semua require_auth agar test tidak perlu wallet nyata
        let contract_id = env.register_contract(None, DocumentContract);
        let client = DocumentContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.init(&admin);
        (env, client, admin)
    }

    // Helper: buat dummy hash 32 byte
    fn dummy_hash(env: &Env, seed: u8) -> BytesN<32> {
        BytesN::from_array(env, &[seed; 32])
    }

    // ── INIT ────────────────────────────────────────────────────────

    #[test]
    fn test_init_sets_admin() {
        let (_, client, admin) = setup();
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    #[should_panic(expected = "AlreadyInitialized")]
    fn test_init_twice_panics() {
        let (env, client, _) = setup();
        let other = Address::generate(&env);
        client.init(&other); // Harus panic
    }

    // ── ISSUE ───────────────────────────────────────────────────────

    #[test]
    fn test_issue_document_success() {
        let (env, client, admin) = setup();
        let hash = dummy_hash(&env, 1);
        client.issue_document(
            &admin,
            &hash,
            &String::from_str(&env, "Budi Santoso"),
            &String::from_str(&env, "Ijazah S1 2024"),
        );
        assert!(client.document_exists(&hash));
        assert_eq!(client.get_doc_count(), 1);
    }

    #[test]
    #[should_panic(expected = "DocAlreadyExists")]
    fn test_issue_duplicate_hash_panics() {
        let (env, client, admin) = setup();
        let hash = dummy_hash(&env, 2);
        let name = String::from_str(&env, "Andi");
        let meta = String::from_str(&env, "S1");
        client.issue_document(&admin, &hash, &name, &meta);
        client.issue_document(&admin, &hash, &name, &meta); // Harus panic
    }

    #[test]
    #[should_panic(expected = "NotAuthorized")]
    fn test_issue_by_non_admin_panics() {
        let (env, client, _) = setup();
        let attacker = Address::generate(&env);
        let hash = dummy_hash(&env, 3);
        client.issue_document(
            &attacker,
            &hash,
            &String::from_str(&env, "Hacker"),
            &String::from_str(&env, "Palsu"),
        );
    }

    // ── VERIFY ──────────────────────────────────────────────────────

    #[test]
    fn test_verify_active_document() {
        let (env, client, admin) = setup();
        let hash = dummy_hash(&env, 4);
        client.issue_document(
            &admin, &hash,
            &String::from_str(&env, "Citra"),
            &String::from_str(&env, "S2"),
        );
        let result = client.verify_document(&hash);
        assert!(result.is_valid);
    }

    #[test]
    fn test_verify_revoked_document_returns_invalid() {
        let (env, client, admin) = setup();
        let hash = dummy_hash(&env, 5);
        client.issue_document(
            &admin, &hash,
            &String::from_str(&env, "Deni"),
            &String::from_str(&env, "S3"),
        );
        client.revoke_document(&admin, &hash);
        let result = client.verify_document(&hash);
        // is_valid harus false, tapi tidak panic
        assert!(!result.is_valid);
        assert!(result.document.revoked_at.is_some());
    }

    #[test]
    #[should_panic(expected = "DocNotFound")]
    fn test_verify_nonexistent_panics() {
        let (env, client, _) = setup();
        client.verify_document(&dummy_hash(&env, 99));
    }

    // ── REVOKE ──────────────────────────────────────────────────────

    #[test]
    fn test_revoke_idempotent() {
        let (env, client, admin) = setup();
        let hash = dummy_hash(&env, 6);
        client.issue_document(
            &admin, &hash,
            &String::from_str(&env, "Eka"),
            &String::from_str(&env, "Diploma"),
        );
        client.revoke_document(&admin, &hash);
        client.revoke_document(&admin, &hash); // Tidak boleh panic
        let doc = client.get_document(&hash);
        assert!(doc.revoked_at.is_some()); // Tetap tercabut
    }

    #[test]
    #[should_panic(expected = "NotAuthorized")]
    fn test_revoke_by_non_admin_panics() {
        let (env, client, admin) = setup();
        let hash = dummy_hash(&env, 7);
        client.issue_document(
            &admin, &hash,
            &String::from_str(&env, "Fani"),
            &String::from_str(&env, "SMA"),
        );
        let attacker = Address::generate(&env);
        client.revoke_document(&attacker, &hash); // Harus panic
    }

    // ── TRANSFER ADMIN ──────────────────────────────────────────────

    #[test]
    fn test_transfer_admin() {
        let (env, client, admin) = setup();
        let new_admin = Address::generate(&env);
        client.transfer_admin(&admin, &new_admin);
        assert_eq!(client.get_admin(), new_admin);
    }

    #[test]
    #[should_panic(expected = "NotAuthorized")]
    fn test_transfer_admin_by_non_admin_panics() {
        let (env, client, _) = setup();
        let faker = Address::generate(&env);
        let new_admin = Address::generate(&env);
        client.transfer_admin(&faker, &new_admin); // Harus panic
    }

    // ── EDGE CASES ──────────────────────────────────────────────────

    #[test]
    fn test_doc_count_increments_correctly() {
        let (env, client, admin) = setup();
        for i in 0..5u8 {
            client.issue_document(
                &admin,
                &dummy_hash(&env, i),
                &String::from_str(&env, "User"),
                &String::from_str(&env, "Meta"),
            );
        }
        assert_eq!(client.get_doc_count(), 5);
    }

    #[test]
    fn test_document_exists_false_for_unknown() {
        let (env, client, _) = setup();
        assert!(!client.document_exists(&dummy_hash(&env, 88)));
    }

    #[test]
    fn test_get_document_raw_shows_revoked_data() {
        let (env, client, admin) = setup();
        let hash = dummy_hash(&env, 10);
        client.issue_document(
            &admin, &hash,
            &String::from_str(&env, "Gilang"),
            &String::from_str(&env, "Profesi"),
        );
        client.revoke_document(&admin, &hash);
        // get_document tidak filter revoke — harus tetap bisa diambil
        let doc = client.get_document(&hash);
        assert!(doc.revoked_at.is_some());
    }
}