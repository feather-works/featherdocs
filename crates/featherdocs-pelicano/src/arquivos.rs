//! Leitura por extensão e gravação atômica sem expor arquivos parcialmente escritos.
use crate::localidade::{tr, trf};
use featherdocs_core::modelo::Documento;
use featherdocs_io::{abrir_na_pasta, Formato};
use std::path::Path;

pub fn ler(p: &Path) -> Result<Documento, String> {
    let formato = p
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Formato::da_extensao)
        .ok_or_else(|| tr("Formato não suportado. Use .fdoz, .docx, .md, .tex ou .zip (projeto LaTeX).").to_string())?;
    let bytes = featherdocs_io::arquivo::ler(p).map_err(|e| match e.kind() {
        std::io::ErrorKind::FileTooLarge => trf("O arquivo {} ultrapassa o limite seguro de 256 MiB.", &[&p.display()]),
        _ => trf("Não consegui abrir {}: {}", &[&p.display(), &e]),
    })?;
    // O .tex pega ao lado dele o que usa: \input, o .bib e as imagens.
    abrir_na_pasta(formato, &bytes, p.parent())
        .map_err(|e| trf("Documento {} inválido: {}", &[&p.display(), &e]))
}
pub fn bytes(doc: &Documento, ext: &str) -> Result<Vec<u8>, String> {
    match Formato::da_extensao(ext) {
        Some(Formato::Fdoz) => featherdocs_io::fdoz::gravar(doc),
        Some(Formato::Docx) => featherdocs_io::docx::gravar(doc),
        Some(Formato::Markdown) => Ok(featherdocs_io::exportar::markdown(doc).into_bytes()),
        Some(Formato::Latex) => Ok(featherdocs_io::exportar::latex(doc).into_bytes()),
        Some(Formato::LatexZip) => featherdocs_io::exportar::latex_zip(doc),
        None => Err(tr("Formato de saída não suportado.").into()),
    }
}
/// O `.fdoz` novo com o histórico do arquivo de onde o documento veio (`origem`: o próprio destino
/// ao salvar, o `.fdoz` aberto num "Salvar como") e uma versão a mais. O destino que é outro arquivo
/// não empresta o histórico dele. Anterior ilegível: começa um histórico novo em vez de não salvar.
pub fn com_historico(novo: Vec<u8>, origem: Option<&Path>) -> Result<Vec<u8>, String> {
    let anterior = origem.filter(|c| c.extension().is_some_and(|e| e.eq_ignore_ascii_case("fdoz"))).and_then(|c| featherdocs_io::arquivo::ler(c).ok());
    let autor = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    let mensagem = tr("Versão salva");
    featherdocs_historico::gravar_com_historico(&novo, anterior.as_deref(), &autor, mensagem)
        .or_else(|_| featherdocs_historico::gravar_com_historico(&novo, None, &autor, mensagem))
}
pub fn gravar(p: &Path, bytes: &[u8]) -> Result<(), String> {
    featherdocs_io::arquivo::gravar(p, bytes).map_err(|e| trf("Não consegui salvar {}: {}", &[&p.display(), &e]))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qa009_arquivo_corrompido_nao_vira_documento_valido() {
        let pasta = std::env::temp_dir();
        let p = pasta.join(format!("featherdocs-qa009-corrompido-{}.fdoz", std::process::id()));
        std::fs::write(&p, b"arquivo invalido e truncado").unwrap();
        let erro = ler(&p).unwrap_err();
        assert!(erro.contains("inválido"), "{erro}");
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn qa009_arquivo_enorme_e_recusado_sem_leitura_completa() {
        let p = std::env::temp_dir().join(format!("featherdocs-qa009-limite-{}.docx", std::process::id()));
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(256 * 1024 * 1024 + 1).unwrap();
        drop(f);
        let erro = ler(&p).unwrap_err();
        assert!(erro.contains("256 MiB"), "{erro}");
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn qa009_arquivo_inexistente_retorna_erro_acionavel() {
        let p = std::env::temp_dir().join(format!(
            "featherdocs-qa009-inexistente-{}.fdoz", std::process::id()
        ));
        let erro = ler(&p).unwrap_err();
        assert!(erro.contains("Não consegui abrir"), "{erro}");
    }

    #[test]
    fn grava_abre_e_falha_preserva_original() {
        let p =
            std::env::temp_dir().join(format!("featherdocs-nativo-{}.fdoz", std::process::id()));
        let mut s = featherdocs_core::sessao::Sessao::default();
        s.inserir_texto("Texto com acentuação");
        let b = bytes(s.documento(), "fdoz").unwrap();
        gravar(&p, &b).unwrap();
        assert_eq!(ler(&p).unwrap(), *s.documento());
        assert!(gravar(&p.join("arquivo"), "não deve substituir".as_bytes()).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b);
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn salvar_duas_vezes_guarda_as_duas_versoes() {
        let p = std::env::temp_dir().join(format!("featherdocs-historico-{}.fdoz", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let mut s = featherdocs_core::sessao::Sessao::default();
        for texto in ["primeira", " segunda"] {
            s.inserir_texto(texto);
            let b = com_historico(bytes(s.documento(), "fdoz").unwrap(), Some(&p)).unwrap();
            gravar(&p, &b).unwrap();
        }
        let b = std::fs::read(&p).unwrap();
        assert_eq!(featherdocs_historico::versoes(&b).unwrap().len(), 2);
        assert_eq!(ler(&p).unwrap(), *s.documento());
        std::fs::remove_file(p).unwrap();
    }
}
