// App.js

function App() {
  const [status, setStatus] = useState(null);

  const handleFileUpload = async (e) => {
    const file = e.target.files[0];
    setStatus("Sedang memverifikasi...");
    
    // Alur: Hitung Hash -> Panggil Kontrak -> Tampilkan Hasil
    const result = await verifyOnChain(myContractClient, file);
    setStatus(result.is_valid ? "VALID ✅" : "TIDAK VALID / DICABUT ❌");
  };

  return (
    <div className="p-10 text-center">
      <h1 className="text-2xl font-bold">StellarTrust Verifier</h1>
      <input type="file" onChange={handleFileUpload} className="mt-5 border p-2" />
      {status && <div className="mt-5 text-xl font-semibold">{status}</div>}
    </div>
  );
}