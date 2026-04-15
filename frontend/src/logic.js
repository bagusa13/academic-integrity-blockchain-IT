// logic.js

import { Buffer } from 'buffer';

// 1. Fungsi Hashing SHA-256 (Mencocokkan sidik jari digital)
export async function hashDocument(file) {
  const arrayBuffer = await file.arrayBuffer();
  const hashBuffer = await crypto.subtle.digest('SHA-256', arrayBuffer);
  // Mengembalikan Uint8Array (32 bytes) sesuai dengan BytesN<32> di Rust
  return new Uint8Array(hashBuffer);
}

// 2. Contoh Logika Verifikasi
export async function verifyOnChain(contractClient, file) {
  const docHash = await hashDocument(file);
  
  // Memanggil fungsi verify_document dari kodingan Rust kamu
  const result = await contractClient.verify_document({ doc_hash: docHash });
  
  if (result.is_valid) {
    console.log("Dokumen Asli! Diterbitkan untuk:", result.document.owner_name);
  } else {
    console.log("Peringatan: Dokumen telah dicabut (Revoked)");
  }
  return result;
}
