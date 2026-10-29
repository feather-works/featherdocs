//! Ler e gravar documentos. Arquivo vindo do disco nunca é confiado: o que está
//! malformado dá erro em português, nunca `panic`.

pub mod base64;
pub mod bib;
pub mod conversao;
pub mod csv;
pub mod dividido;
pub mod arquivo;
pub mod docx;
mod docx_comentarios;
mod docx_fontes;
mod docx_gravar;
pub mod exportar;
pub mod fdoz;
pub mod grafico_docs;
pub mod latex;
pub mod markdown;
pub mod realce;

use featherdocs_core::modelo::Documento;

/// Os formatos que o FeatherDocs lê.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formato {
    /// O contêiner próprio (ZIP + JSON).
    Fdoz,
    Markdown,
    Latex,
    /// Word e Google Docs (`.docx`).
    Docx,
    /// Projeto LaTeX num `.zip` (o "Baixar fonte" do Overleaf): o `.tex` principal e o que ele usa.
    LatexZip,
}

impl Formato {
    /// O formato de uma extensão (sem o ponto, qualquer caixa).
    pub fn da_extensao(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "fdoz" => Some(Self::Fdoz),
            "md" | "markdown" | "mdown" => Some(Self::Markdown),
            "tex" | "latex" => Some(Self::Latex),
            "docx" => Some(Self::Docx),
            "zip" => Some(Self::LatexZip),
            _ => None,
        }
    }
}

/// Lê um documento nos bytes de um arquivo.
pub fn abrir(formato: Formato, bytes: &[u8]) -> Result<Documento, String> {
    abrir_na_pasta(formato, bytes, None)
}

/// Como `abrir`; o `.tex` pega na `pasta` dele o que usa (`\input`, `.bib`, imagens).
pub fn abrir_na_pasta(formato: Formato, bytes: &[u8], pasta: Option<&std::path::Path>) -> Result<Documento, String> {
    match formato {
        Formato::Fdoz => fdoz::ler(bytes),
        Formato::Markdown => Ok(markdown::ler(&texto_utf8(bytes)?)),
        Formato::Latex => Ok(latex::ler_na_pasta(&texto_utf8(bytes)?, pasta).documento),
        Formato::LatexZip => latex::projeto::ler_zip(bytes).map(|(l, _)| l.documento),
        Formato::Docx => docx::ler(bytes),
    }
}

/// Teto do que uma parte de um ZIP vindo de fora (`.docx`, `.fdoz`) pode ocupar descompactada:
/// o tamanho declarado no ZIP pode mentir, então o teto vale para o que sai de fato.
// shortcut: o teto é por parte; um .docx com centenas de imagens-bomba ainda soma várias vezes o teto. Somar por arquivo se aparecer.
pub(crate) const LIMITE_PARTE: u64 = if cfg!(test) { 1 << 20 } else { 256 << 20 };

/// Lê uma parte de ZIP até [`LIMITE_PARTE`]; passou dele é erro (a "bomba de ZIP").
pub(crate) fn ler_limitado(r: impl std::io::Read) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut v = Vec::new();
    r.take(LIMITE_PARTE + 1).read_to_end(&mut v)?;
    if v.len() as u64 > LIMITE_PARTE {
        return Err(std::io::Error::other(format!("passa de {} MiB descompactada", LIMITE_PARTE >> 20)));
    }
    Ok(v)
}

/// Texto UTF-8 (com ou sem BOM); outra codificação dá erro claro em vez de lixo.
pub(crate) fn texto_utf8(bytes: &[u8]) -> Result<String, String> {
    let sem_bom = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8(sem_bom.to_vec()).map_err(|_| "O arquivo não está em UTF-8. Salve-o como UTF-8 e tente de novo.".to_string())
}
