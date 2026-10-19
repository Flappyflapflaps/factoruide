//! Zip archives of a few files (the addon, a Polestar pack) for the page to download: stored,
//! not compressed (the browser downloads them compressed anyway).

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A zip of `files` (path inside the archive, content).
pub fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    // 1980-01-01, the earliest date a zip can hold.
    let (time, date) = (0u16, (1u16 << 5) | 1);
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes());
            // UTF-8 names.
            h.extend_from_slice(&(1u16 << 11).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            h.extend_from_slice(&time.to_le_bytes());
            h.extend_from_slice(&date.to_le_bytes());
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&size.to_le_bytes());
            h.extend_from_slice(&size.to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            if central {
                // Comment length, disk, internal and external attributes.
                h.extend_from_slice(&[0; 10]);
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend(header(0x0201_4b50, true));
    }
    let (start, size) = (out.len() as u32, central.len() as u32);
    out.extend(central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc_of_the_check_string() {
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
    }

    /// `unzip` reads the archive back, when it is installed.
    #[test]
    fn unzip_reads_it_back() {
        let dir = std::env::temp_dir().join(format!("fg-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("t.zip");
        let files = [
            ("a/b.txt".to_owned(), b"hello".to_vec()),
            ("c.lua".to_owned(), vec![7; 1000]),
        ];
        std::fs::write(&file, super::zip(&files)).unwrap();
        if let Ok(out) = std::process::Command::new("unzip")
            .arg("-p")
            .arg(&file)
            .arg("a/b.txt")
            .output()
        {
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(out.stdout, b"hello");
            let test = std::process::Command::new("unzip")
                .arg("-t")
                .arg(&file)
                .output()
                .unwrap();
            assert!(test.status.success(), "{}", String::from_utf8_lossy(&test.stdout));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn archive_ends_with_the_directory_of_its_files() {
        let z = super::zip(&[("a/b.txt".into(), b"hello".to_vec())]);
        assert_eq!(&z[..4], b"PK\x03\x04");
        assert_eq!(&z[z.len() - 22..z.len() - 18], b"PK\x05\x06");
        assert_eq!(u16::from_le_bytes([z[z.len() - 12], z[z.len() - 11]]), 1);
    }
}
