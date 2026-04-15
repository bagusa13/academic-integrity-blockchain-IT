# 🚀 StellarTrust: Decentralized Document Verifier

### **Overview**
**StellarTrust** is a decentralized application (dApp) built on the **Stellar Network** using **Soroban Smart Contracts**. It provides a robust, transparent, and immutable solution for verifying the authenticity of digital documents such as academic diplomas, professional certifications, and legal records.

---

### **Project Description**
In an era where digital forgery is becoming increasingly sophisticated, verifying the legitimacy of digital credentials has become a major challenge for institutions and employers. **StellarTrust** addresses this by using a "Digital Fingerprint" approach. Instead of storing sensitive documents directly on the blockchain, the system only records the **cryptographic hash (SHA-256)** of the file.

When a document is issued, its hash is permanently etched into the Stellar ledger. Anyone holding the original file can later prove its authenticity through the StellarTrust web portal. The application re-calculates the hash of the uploaded file locally and compares it with the records on-chain. This ensures **Privacy-by-Design**, as the actual content of the document remains off-chain, while its integrity is guaranteed by the Stellar network.

---

### ✨ **Key Features**
* **Immutable Issuance:** Only the designated administrator can issue new document records.
* **Instant Public Verification:** A zero-cost, high-speed verification process available to any third party.
* **Cryptographic Integrity:** Utilizes SHA-256 hashing to ensure that even a single-pixel change in a document will result in failure.
* **Revocation Management:** Ability to invalidate documents in real-time if they are no longer valid.
* **On-Chain Audit Trail:** Every action is recorded with a timestamp for full transparency.

---

### ⛓️ **Smart Contract Technical Info**
* **Network:** Stellar Testnet
* **Contract ID:** `CCEJHT4TQF5K2WPLWB72P2IG4GVFD6PZRNAV6V6L3OK6ZKHXW4AZDOU7`
* **Admin Address:** `GA7P5B2KOAF3N2JKZFBDNB7ZIGO6MVYMXVR5JSIL6ZZ3GAZQWKMWG3I2`
* **Wasm Hash:** `3683d35a86cffc92d7f6224d4fd75f28c8380679a0d2cc01e6d9545b1a468e45`

---

### 🛠️ **Technological Stack**
* **Smart Contract:** Rust & Soroban SDK.
* **Frontend:** React.js.
* **Interaction:** Soroban-Client SDK.
* **Security:** Web Crypto API for client-side hashing.

---

### 👨‍💻 **Developer Profile**
* **Name:** Mohammad Bagus Satrio
* **NIM:** 103032400099
* **Major:** Information Technology
* **Organization:** HMIT (Himpunan Mahasiswa Teknologi Informasi)

---

> This project was built to demonstrate secure, decentralized identity and credentialing systems on the Stellar network.