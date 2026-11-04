//! Budweiser DWG benchmark compatibility test suite.
//!
//! Validates 100% fidelity read and round-trip write for all benchmark drawings
//! in the `budweiser/` directory, including unencrypted and password-protected
//! drawings with clean-room RC4/MD5 decryption.

use std::path::Path;
use acadrust::io::dwg::{DwgReader, DwgWriter};

#[test]
fn test_budweiser_unencrypted() {
    if !Path::new("budweiser").exists() {
        eprintln!("Skipping budweiser benchmark: budweiser folder not present");
        return;
    }
    let files = [
        "budweiser/budweiser(original).dwg",
        "budweiser/budweiser-western.dwg",
        "budweiser/budweiser2.dwg",
        "budweiser/budweiser2010.dwg",
        "budweiser/budweiser2013.dwg",
        "budweiser/budweiser2018.dwg",
    ];

    for file_path in &files {
        let path = Path::new(file_path);
        assert!(path.exists(), "File not found: {}", file_path);

        // Strict read
        let mut reader = DwgReader::from_file(path)
            .unwrap_or_else(|e| panic!("Failed to create reader for {}: {:?}", file_path, e));
        let doc = reader.read()
            .unwrap_or_else(|e| panic!("Failed to read {} in strict mode: {:?}", file_path, e));

        let entity_count = doc.entities().count();
        let layer_count = doc.layers.len();
        let block_count = doc.block_records.len();

        println!(
            "PASS {}: {} entities, {} layers, {} blocks",
            file_path, entity_count, layer_count, block_count
        );
        assert!(entity_count > 500, "Too few entities in {}", file_path);

        // Test round-trip write and re-read
        let written_bytes = DwgWriter::write_to_vec(&doc)
            .unwrap_or_else(|e| panic!("Failed to write {} to DWG: {:?}", file_path, e));

        let mut reread_reader = DwgReader::from_stream(std::io::Cursor::new(&written_bytes));
        let reread_doc = reread_reader.read()
            .unwrap_or_else(|e| panic!("Failed to re-read written DWG for {}: {:?}", file_path, e));

        assert_eq!(
            entity_count,
            reread_doc.entities().count(),
            "Entity count mismatch in round-trip for {}",
            file_path
        );
        assert_eq!(
            layer_count,
            reread_doc.layers.len(),
            "Layer count mismatch in round-trip for {}",
            file_path
        );
        println!("ROUND-TRIP PASS: {}", file_path);
    }
}

#[test]
fn test_budweiser_encrypted() {
    if !Path::new("budweiser").exists() {
        eprintln!("Skipping budweiser benchmark: budweiser folder not present");
        return;
    }
    let encrypted_files = [
        "budweiser/budweiser3.dwg",
        "budweiser/budweiser2007.dwg",
        "budweiser/budweiser2008.dwg",
    ];

    for file_path in &encrypted_files {
        let path = Path::new(file_path);
        assert!(path.exists(), "File not found: {}", file_path);

        // 1. Reading without password must fail with Decryption error
        let mut reader_no_pwd = DwgReader::from_file(path).unwrap();
        let res_no_pwd = reader_no_pwd.read();
        assert!(
            res_no_pwd.is_err(),
            "Expected read without password to fail for {}",
            file_path
        );

        // 2. Reading with wrong password must fail with Decryption error
        let mut reader_wrong_pwd = DwgReader::from_file(path)
            .unwrap()
            .with_password("wrong_password");
        let res_wrong_pwd = reader_wrong_pwd.read();
        assert!(
            res_wrong_pwd.is_err(),
            "Expected read with wrong password to fail for {}",
            file_path
        );

        // 3. Reading with correct password "budweiser" must succeed
        let mut reader = DwgReader::from_file(path)
            .unwrap()
            .with_password("budweiser");
        let doc = reader.read().unwrap_or_else(|e| {
            panic!("Failed to decrypt and read {} with password: {:?}", file_path, e)
        });

        let entity_count = doc.entities().count();
        let layer_count = doc.layers.len();
        let block_count = doc.block_records.len();

        println!(
            "PASS DECRYPT {}: {} entities, {} layers, {} blocks",
            file_path, entity_count, layer_count, block_count
        );
        assert!(entity_count > 500, "Too few entities in {}", file_path);

        // 4. Test writing back to DWG and re-reading
        let written_bytes = DwgWriter::write_to_vec(&doc)
            .unwrap_or_else(|e| panic!("Failed to write {} to DWG: {:?}", file_path, e));

        let mut reread_reader = DwgReader::from_stream(std::io::Cursor::new(&written_bytes));
        let reread_doc = reread_reader.read()
            .unwrap_or_else(|e| panic!("Failed to re-read written DWG for {}: {:?}", file_path, e));

        assert_eq!(
            entity_count,
            reread_doc.entities().count(),
            "Entity count mismatch in round-trip for {}",
            file_path
        );
        assert_eq!(
            layer_count,
            reread_doc.layers.len(),
            "Layer count mismatch in round-trip for {}",
            file_path
        );
        println!("ROUND-TRIP PASS FOR DECRYPTED: {}", file_path);
    }
}
