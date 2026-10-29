//! Arquivo malformado dá erro ou documento, nunca `panic` nem laço infinito.

use featherdocs_io::{abrir, Formato};

/// Gerador pseudoaleatório fixo (reproduzível), sem dependência.
struct Sorteio(u64);

impl Sorteio {
    fn proximo(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

const PEDACOS_TEX: [&str; 24] = [
    "\\section{", "}", "{", "\\begin{itemize}", "\\end{itemize}", "\\item ", "\\textbf{", "$", "$$", "\\[", "\\]", "\\\\", "&", "%", "\\", "texto ", "\n\n", "\\begin{tabular}{ll}", "\\end{tabular}", "\\'", "\\href{", "~", "--", "é",
];
const PEDACOS_MD: [&str; 22] = ["# ", "- ", "1. ", "> ", "```", "**", "*", "`", "[", "](", ")", "|", "---", "$", "$$", "![", "\n", "\n\n", "texto ", "  ", "~~", "é"];

fn sorteado(pedacos: &[&str], semente: u64, n: usize) -> String {
    let mut s = Sorteio(semente);
    (0..n).map(|_| pedacos[s.proximo(pedacos.len())]).collect()
}

#[test]
fn latex_aleatorio_nao_quebra() {
    for semente in 0..300 {
        let fonte = sorteado(&PEDACOS_TEX, semente, 60);
        let _ = abrir(Formato::Latex, fonte.as_bytes());
    }
}

#[test]
fn markdown_aleatorio_nao_quebra() {
    for semente in 0..300 {
        let fonte = sorteado(&PEDACOS_MD, semente, 60);
        let _ = abrir(Formato::Markdown, fonte.as_bytes());
    }
}

#[test]
fn bytes_aleatorios_nao_quebram_nenhum_leitor() {
    for semente in 0..100 {
        let mut s = Sorteio(semente);
        let bytes: Vec<u8> = (0..200).map(|_| s.proximo(256) as u8).collect();
        for f in [Formato::Fdoz, Formato::Docx, Formato::Markdown, Formato::Latex] {
            let _ = abrir(f, &bytes);
        }
    }
}

#[test]
fn texto_que_nao_e_utf8_da_erro_claro() {
    let erro = abrir(Formato::Markdown, &[0xff, 0xfe, 0x41]).unwrap_err();
    assert!(erro.contains("UTF-8"));
}

/// Roda `f` numa thread de pilha pequena (como a do app, que abre arquivos fora da principal) e
/// com prazo: estourar a pilha ou demorar demais é falha.
fn com_pilha_pequena(f: impl FnOnce() + Send + 'static) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new().stack_size(2 << 20).spawn(move || {
        f();
        let _ = tx.send(());
    }).unwrap();
    rx.recv_timeout(std::time::Duration::from_secs(20)).expect("estourou a pilha ou demorou demais");
}

#[test]
fn latex_com_macro_que_cresce_termina() {
    com_pilha_pequena(|| {
        let bomba = "\\documentclass{article}\\def\\a{\\a\\a\\a\\a\\a\\a\\a\\a\\a\\a}\\begin{document}\\a\\end{document}";
        let _ = abrir(Formato::Latex, bomba.as_bytes());
    });
}

#[test]
fn latex_com_listas_fundas_nao_estoura() {
    com_pilha_pequena(|| {
        let listas = format!("{}x{}", "\\begin{itemize}\\item ".repeat(400), "\\end{itemize}".repeat(400));
        let _ = abrir(Formato::Latex, listas.as_bytes());
    });
}

#[test]
fn latex_com_chaves_fundas_nao_estoura() {
    com_pilha_pequena(|| {
        let chaves = format!("{}x{}", "{".repeat(50_000), "}".repeat(50_000));
        let _ = abrir(Formato::Latex, chaves.as_bytes());
    });
}

/// Um ZIP válido com o XML do Word por dentro estragado ou fundo demais: erro ou documento, sem cair.
#[test]
fn docx_com_tabelas_fundas_e_lista_255_nao_quebra() {
    com_pilha_pequena(|| {
        let fundo = format!("{}<w:p/>{}", "<w:tbl><w:tr><w:tc>".repeat(1000), "</w:tc></w:tr></w:tbl>".repeat(1000));
        for corpo in [fundo, "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"255\"/><w:numId w:val=\"1\"/></w:numPr></w:pPr><w:r><w:t>x</w:t></w:r></w:p>".to_string(), "<w:p><w:r><w:t>sem fim".to_string()] {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            zip.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zip, format!("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{corpo}</w:body></w:document>").as_bytes()).unwrap();
            let bytes = zip.finish().unwrap().into_inner();
            if let Ok(d) = abrir(Formato::Docx, &bytes) {
                let fdoz = featherdocs_io::fdoz::gravar(&d).unwrap();
                featherdocs_io::fdoz::ler(&fdoz).unwrap();
            }
        }
    });
}
