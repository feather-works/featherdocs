//! Ida e volta de um `.docx` gravado pelo LibreOffice (`dados/libreoffice_bateria.docx`, feito de
//! `libreoffice_bateria.fodt` com `libreoffice --headless --convert-to docx`): títulos, listas
//! aninhadas, tabela mesclada, cabeçalho e rodapé, notas, imagem e seção em paisagem.

use featherdocs_core::modelo::Numeracao;
use featherdocs_io::docx;
use std::io::Read;

const BATERIA: &[u8] = include_bytes!("dados/libreoffice_bateria.docx");

fn parte(docx: &[u8], nome: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(docx)).unwrap();
    let mut s = String::new();
    z.by_name(nome).unwrap().read_to_string(&mut s).unwrap();
    s
}

#[test]
fn o_docx_do_libreoffice_volta_sem_perder() {
    let doc = docx::ler(BATERIA).unwrap();
    // O LibreOffice liga os títulos a uma numeração de formato "none": não são títulos numerados.
    assert_eq!(doc.estilos.numeracao, Numeracao::Nenhuma);
    // 1440 twips na primeira seção; a seção em paisagem (sem cabeçalho) grava 0 e não apaga.
    assert_eq!(doc.extras.documento.distancia_cabecalho, 72.0);
    assert_eq!(doc.extras.documento.distancia_rodape, 72.0);

    let volta = docx::gravar(&doc).unwrap();
    let corpo = parte(&volta, "word/document.xml");
    for texto in ["Título Um", "A1B1 mesclada", "C1C2 vertical", "Sub a", "Bala dois", "ZQS"] {
        assert!(corpo.contains(texto), "{texto} sumiu");
    }
    assert!(corpo.contains("<w:gridSpan w:val=\"2\"/>") && corpo.contains("<w:vMerge"));
    // Margem das células 0 e recuo -5 do LibreOffice: sem eles a tabela voltava ~1,4 mm para a direita.
    assert!(corpo.contains("<w:tblInd w:w=\"-5\" w:type=\"dxa\"/>"), "recuo da tabela sumiu");
    assert!(corpo.contains("<w:tblCellMar><w:left w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"0\" w:type=\"dxa\"/>"), "margem das células voltou à padrão");
    assert!(corpo.contains("w:orient=\"landscape\"") && corpo.contains("<w:drawing>"));
    // A nota sai com o estilo de nota (letra menor), que o styles.xml define.
    assert!(parte(&volta, "word/footnotes.xml").contains("<w:pStyle w:val=\"FootnoteText\"/>"));
    assert!(parte(&volta, "word/endnotes.xml").contains("<w:pStyle w:val=\"EndnoteText\"/>"));
    let estilos = parte(&volta, "word/styles.xml");
    assert!(estilos.contains("w:styleId=\"FootnoteText\"") && estilos.contains("w:styleId=\"EndnoteText\""));

    // E a segunda volta dá o mesmo.
    let de_novo = docx::ler(&volta).unwrap();
    assert_eq!(de_novo.estilos.numeracao, Numeracao::Nenhuma);
    assert_eq!(de_novo.extras.documento.distancia_cabecalho, 72.0);
    assert_eq!(de_novo.blocos.len(), doc.blocos.len());
}
