//! Ler e gravar o arquivo do documento no disco, para o aplicativo e para o servidor MCP: a leitura
//! tem teto e só aceita arquivo comum; a gravação é atômica (nunca expõe um arquivo pela metade).
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// O maior arquivo que se abre.
pub const LIMITE: u64 = 256 * 1024 * 1024;

/// Os bytes do arquivo `p`. Recusa o que não é arquivo comum (um FIFO travaria a leitura) e o que
/// passa de [`LIMITE`] (com `ErrorKind::FileTooLarge`), sem ler tudo antes.
pub fn ler(p: &Path) -> std::io::Result<Vec<u8>> {
    let grande = || std::io::Error::new(std::io::ErrorKind::FileTooLarge, format!("o arquivo passa de {} MiB", LIMITE >> 20));
    let arquivo = std::fs::File::open(p)?;
    let meta = arquivo.metadata()?;
    if !meta.is_file() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "não é um arquivo comum"));
    }
    if meta.len() > LIMITE {
        return Err(grande());
    }
    let mut bytes = Vec::new();
    arquivo.take(LIMITE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMITE {
        return Err(grande());
    }
    Ok(bytes)
}

/// Grava `bytes` em `p` por um arquivo temporário na mesma pasta, gravado até o disco e renomeado
/// por cima; o arquivo de antes fica inteiro até o fim e as permissões dele continuam.
pub fn gravar(p: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pasta = p.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp: PathBuf = pasta.join(format!(".featherdocs-{}-{seq}.tmp", std::process::id()));
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let resultado = (|| {
        f.write_all(bytes)?;
        if let Ok(antes) = std::fs::metadata(p) {
            f.set_permissions(antes.permissions())?;
        }
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, p)
    })();
    if resultado.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    resultado
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grava_por_cima_mantendo_as_permissoes_e_le_de_volta() {
        let p = std::env::temp_dir().join(format!("featherdocs-arquivo-{}.md", std::process::id()));
        std::fs::write(&p, b"antes").unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
        gravar(&p, b"depois").unwrap();
        assert_eq!(ler(&p).unwrap(), b"depois");
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&p).unwrap().permissions()) & 0o777, 0o600);
        std::fs::remove_file(&p).unwrap();
        assert!(ler(&std::env::temp_dir()).is_err(), "pasta não é arquivo");
    }
}
