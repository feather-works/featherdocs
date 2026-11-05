//! Recuperação depois de uma queda: enquanto o documento tem mudanças não salvas, uma cópia .fdoz
//! vai para `~/.local/share/featherdocs/recuperacao/` (no máximo a cada `INTERVALO`). Salvar,
//! descartar ou fechar sem pendência apaga a cópia; se o app cair, ela fica e é oferecida ao abrir.
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const INTERVALO: Duration = Duration::from_secs(20);

/// Uma cópia deixada por um FeatherDocs que não fechou direito.
#[derive(Clone, Debug, PartialEq)]
pub struct Copia {
    pub arquivo: PathBuf,
    /// O arquivo de onde o documento veio, se tinha um.
    pub original: Option<PathBuf>,
    pub quando: SystemTime,
}

pub fn pasta() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))?;
    Some(base.join("featherdocs/recuperacao"))
}

/// O nome da cópia desta execução do app (o processo e a hora em que abriu).
pub fn nome_da_sessao() -> String {
    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis();
    format!("{}-{t}", std::process::id())
}

/// Grava a cópia sem deixar arquivo pela metade nem vazio depois de uma queda de energia: o
/// `arquivo::gravar` leva os bytes até o disco antes de renomear.
pub fn gravar(pasta: &Path, nome: &str, fdoz: &[u8], original: Option<&Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(pasta)?;
    featherdocs_io::arquivo::gravar(&pasta.join(format!("{nome}.fdoz")), fdoz)?;
    let origem = pasta.join(format!("{nome}.origem"));
    match original {
        Some(p) => std::fs::write(origem, p.to_string_lossy().as_bytes()),
        None => std::fs::remove_file(origem).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }),
    }
}

/// Apaga a cópia `nome` (e o caminho de origem dela); não existir não é erro.
pub fn apagar(pasta: &Path, nome: &str) {
    for ext in ["fdoz", "fdoz.tmp", "origem"] {
        let _ = std::fs::remove_file(pasta.join(format!("{nome}.{ext}")));
    }
}

/// As cópias na pasta, menos a desta sessão, da mais nova para a mais velha.
pub fn listar(pasta: &Path, menos: &str) -> Vec<Copia> {
    let Ok(dir) = std::fs::read_dir(pasta) else { return Vec::new() };
    let mut v: Vec<Copia> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "fdoz") && p.file_stem().is_some_and(|s| s != menos))
        .map(|arquivo| {
            let original = std::fs::read_to_string(arquivo.with_extension("origem")).ok().filter(|s| !s.is_empty()).map(PathBuf::from);
            let quando = std::fs::metadata(&arquivo).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            Copia { arquivo, original, quando }
        })
        .collect();
    v.sort_by(|a, b| b.quando.cmp(&a.quando));
    v
}

/// Apaga a cópia `c` (a que foi recuperada ou descartada).
pub fn descartar(c: &Copia) {
    if let (Some(pasta), Some(nome)) = (c.arquivo.parent(), c.arquivo.file_stem().and_then(|s| s.to_str())) {
        apagar(pasta, nome);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grava_lista_e_apaga_as_copias() {
        let pasta = std::env::temp_dir().join(format!("featherdocs-recuperacao-{}", nome_da_sessao()));
        gravar(&pasta, "minha", b"um", None).unwrap();
        gravar(&pasta, "outra", b"dois", Some(Path::new("/tmp/carta.fdoz"))).unwrap();
        // Regravar troca o conteúdo e não deixa o temporário.
        gravar(&pasta, "outra", b"tres", Some(Path::new("/tmp/carta.fdoz"))).unwrap();
        assert!(!pasta.join("outra.fdoz.tmp").exists());
        let v = listar(&pasta, "minha");
        assert_eq!(v.len(), 1, "a cópia desta sessão não é oferecida");
        assert_eq!(v[0].original.as_deref(), Some(Path::new("/tmp/carta.fdoz")));
        assert_eq!(std::fs::read(&v[0].arquivo).unwrap(), b"tres");
        descartar(&v[0]);
        apagar(&pasta, "minha");
        assert!(listar(&pasta, "").is_empty());
        let _ = std::fs::remove_dir(&pasta);
    }
}
