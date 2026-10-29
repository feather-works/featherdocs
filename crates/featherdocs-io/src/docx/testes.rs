use super::*;
use featherdocs_core::edicao::nome_do_tipo;
use featherdocs_core::inline::{analisar, texto_puro};
use featherdocs_core::modelo::{Alinhamento, Bloco, Extras, ITALICO, NEGRITO, SUBLINHADO, TACHADO};
use std::io::Write;

/// Monta um `.docx` mínimo com as partes dadas.
fn docx(partes: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (nome, xml) in partes {
        zip.start_file(*nome, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

const W: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"";

fn corpo(xml: &str) -> Vec<u8> {
    docx(&[
        ("word/document.xml", &format!("<w:document {W}><w:body>{xml}</w:body></w:document>")),
        ("word/styles.xml", &format!("<w:styles {W}><w:style w:styleId=\"Ttulo1\"><w:name w:val=\"heading 1\"/></w:style><w:style w:styleId=\"Citao\"><w:name w:val=\"Quote\"/></w:style><w:style w:styleId=\"Meu\"><w:name w:val=\"Meu título\"/><w:basedOn w:val=\"Ttulo1\"/></w:style></w:styles>")),
        ("word/numbering.xml", &format!("<w:numbering {W}><w:abstractNum w:abstractNumId=\"0\"><w:lvl w:ilvl=\"0\"><w:numFmt w:val=\"decimal\"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId=\"1\"><w:lvl w:ilvl=\"0\"><w:numFmt w:val=\"bullet\"/></w:lvl></w:abstractNum><w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num><w:num w:numId=\"2\"><w:abstractNumId w:val=\"1\"/></w:num></w:numbering>")),
        ("word/_rels/document.xml.rels", "<Relationships><Relationship Id=\"rId9\" Target=\"http://x.org\"/><Relationship Id=\"rId8\" Target=\"media/a.png\"/></Relationships>"),
        ("word/media/a.png", "PNG"),
        ("docProps/core.xml", "<cp:coreProperties xmlns:cp=\"c\" xmlns:dc=\"d\"><dc:title>Relatório</dc:title><dc:creator>Ana</dc:creator></cp:coreProperties>"),
    ])
}

#[test]
fn titulo_paragrafo_e_formatacao() {
    let d = ler(&corpo(
        "<w:p><w:pPr><w:pStyle w:val=\"Ttulo1\"/></w:pPr><w:r><w:t>Intro</w:t></w:r></w:p>\
         <w:p><w:r><w:t xml:space=\"preserve\">Um </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>forte</w:t></w:r><w:r><w:t> &amp; fim</w:t></w:r></w:p>\
         <w:p/>\
         <w:p><w:pPr><w:pStyle w:val=\"Meu\"/></w:pPr><w:r><w:t>Herdado</w:t></w:r></w:p>",
    ))
    .unwrap();
    assert_eq!(d.meta.titulo, "Relatório");
    assert_eq!(d.meta.autor, "Ana");
    assert_eq!(d.blocos.len(), 4, "o parágrafo vazio fica (é espaço que o autor pôs)");
    assert!(matches!(d.blocos[0].conteudo, Conteudo::Titulo { nivel: 1, .. }));
    let Conteudo::Paragrafo { texto } = &d.blocos[1].conteudo else { panic!() };
    assert_eq!(texto_puro(texto), "Um forte & fim");
    assert_eq!(texto[1].estilo, NEGRITO);
    assert!(matches!(d.blocos[3].conteudo, Conteudo::Titulo { nivel: 1, .. }), "estilo baseado em título é título");
}

#[test]
fn listas_com_marcador_e_numeradas() {
    let item = |num: &str, t: &str| format!("<w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{num}\"/></w:numPr></w:pPr><w:r><w:t>{t}</w:t></w:r></w:p>");
    let d = ler(&corpo(&format!("{}{}", item("1", "um"), item("2", "ponto")))).unwrap();
    assert_eq!(d.blocos.iter().map(|b| nome_do_tipo(&b.conteudo)).collect::<Vec<_>>(), vec!["item_ordenado", "item"]);
}

#[test]
fn tabela_link_formula_e_quebra() {
    let d = ler(&corpo(
        "<w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr>\
         <w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr></w:tbl>\
         <w:p><w:hyperlink r:id=\"rId9\"><w:r><w:t>site</w:t></w:r></w:hyperlink></w:p>\
         <w:p><m:oMath><m:r><m:t>x^2</m:t></m:r></m:oMath></w:p>\
         <w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>\
         <w:p><w:r><w:t>depois</w:t></w:r></w:p>",
    ))
    .unwrap();
    let Conteudo::Tabela { cabecalho, linhas } = &d.blocos[0].conteudo else { panic!() };
    assert!(*cabecalho);
    assert_eq!(linhas.len(), 2);
    let Conteudo::Paragrafo { texto } = &d.blocos[1].conteudo else { panic!() };
    assert_eq!(texto[0].link.as_deref(), Some("http://x.org"));
    let Conteudo::Paragrafo { texto } = &d.blocos[2].conteudo else { panic!() };
    assert_eq!(texto[0].estilo, MATEMATICA);
    assert!(matches!(d.blocos[3].conteudo, Conteudo::QuebraDePagina));
}

#[test]
fn imagem_vira_bloco_com_o_arquivo_embutido() {
    let d = ler(&corpo("<w:p><w:r><w:drawing><wp:extent cx=\"2540000\" cy=\"1270000\"/><a:blip r:embed=\"rId8\"/></w:drawing></w:r></w:p>")).unwrap();
    let Conteudo::Imagem { origem, largura, .. } = &d.blocos[0].conteudo else { panic!("{:?}", d.blocos) };
    assert_eq!(origem, &crate::base64::para_data_url("image/png", b"PNG"));
    assert!((*largura - 200.0).abs() < 0.1);
}

#[test]
fn arquivo_errado_da_erro_em_portugues() {
    assert!(ler(b"nao e zip").unwrap_err().contains(".docx"));
    assert!(ler(&docx(&[("x.txt", "oi")])).unwrap_err().contains("document.xml"));
    assert!(ler(&docx(&[("word/document.xml", "<w:p><w:r></w:p>")])).is_err());
}

// ---------------------------------------------------------------------------
// Ida e volta: o que o FeatherDocs grava, ele lê igual
// ---------------------------------------------------------------------------

fn ida_e_volta(d: &Documento) -> Documento {
    ler(&gravar(d).unwrap()).unwrap()
}

/// Os blocos sem o `id` (o leitor numera de novo).
fn sem_ids(d: &Documento) -> Vec<(Conteudo, Atrib)> {
    d.blocos.iter().map(|b: &Bloco| (b.conteudo.clone(), b.atrib)).collect()
}

#[test]
fn volta_igual_texto_e_formatacao() {
    use featherdocs_core::modelo::Fmt;
    let mut d = Documento::vazio();
    d.meta.titulo = "Título & cia".into();
    d.meta.autor = "Ana".into();
    d.acrescentar(Conteudo::Titulo { nivel: 2, texto: analisar("Oi **você**") });
    let rico = vec![
        Trecho::simples("normal "),
        Trecho { texto: "serifa grande".into(), estilo: SERIFA | ITALICO, fmt: Fmt { tamanho: 20.0, cor: 0xFF00_00FF, realce: 0xFFFF_00FF, posicao: 0, ..Fmt::default() }, ..Trecho::default() },
        Trecho { texto: "2".into(), fmt: Fmt { posicao: 1, ..Fmt::default() }, ..Trecho::default() },
        Trecho { texto: " sub".into(), estilo: SUBLINHADO | TACHADO, ..Trecho::default() },
        Trecho { texto: "site".into(), link: Some("https://x.org/?a=1&b=2".into()), ..Trecho::default() },
        Trecho { texto: "E=mc^2".into(), estilo: MATEMATICA, ..Trecho::default() },
    ];
    d.acrescentar(Conteudo::Paragrafo { texto: rico });
    d.acrescentar(Conteudo::Paragrafo { texto: Vec::new() });
    d.acrescentar(Conteudo::Citacao { texto: analisar("citado") });
    d.acrescentar(Conteudo::Codigo { linguagem: String::new(), codigo: "fn main() {\n\n    x < 1\n}".into() });
    let p = &mut d.blocos[1].atrib;
    p.alinhamento = Alinhamento::Justificado;
    p.entrelinha = 1.5;
    p.antes = Some(12.0);
    p.depois = Some(6.0);
    p.recuo_esq = 36.0;
    p.recuo_dir = 18.0;
    p.primeira = -18.0;
    p.fundo = 0xDDEE_FFFF;
    p.borda = 1 | 8;
    p.cor_borda = 0x3366_99FF;
    let volta = ida_e_volta(&d);
    assert_eq!(volta.meta, d.meta);
    assert_eq!(sem_ids(&volta), sem_ids(&d));
}

#[test]
fn volta_igual_listas_tabela_equacao_e_especiais() {
    let mut d = Documento::vazio();
    let item = |d: &mut Documento, ordenado, nivel, t: &str, lista: Lista| {
        d.acrescentar(Conteudo::Item { ordenado, nivel, texto: analisar(t) });
        d.blocos.last_mut().unwrap().atrib.lista = lista;
    };
    item(&mut d, true, 0, "um", Lista::Padrao);
    item(&mut d, true, 1, "um.um", Lista::Alfabetica);
    item(&mut d, false, 0, "ponto", Lista::Padrao);
    item(&mut d, false, 1, "círculo", Lista::Circulo);
    item(&mut d, false, 0, "seta", Lista::Seta);
    item(&mut d, false, 0, "tarefa", Lista::Tarefa);
    d.blocos.last_mut().unwrap().atrib.marcado = true;
    item(&mut d, true, 0, "romano", Lista::Romana);
    item(&mut d, true, 0, "romano minúsculo", Lista::RomanaMinuscula);
    d.acrescentar(Conteudo::Tabela { cabecalho: true, linhas: vec![vec![analisar("a"), analisar("**b**")], vec![analisar("1"), analisar("2")]] });
    d.acrescentar(Conteudo::Equacao { latex: "\\frac{a}{b}".into() });
    d.acrescentar(Conteudo::Regua);
    d.acrescentar(Conteudo::QuebraDePagina);
    d.acrescentar(Conteudo::sumario(2));
    d.acrescentar(Conteudo::Sumario { niveis: 3, sem_numeros: true, sem_pontilhado: false, lista: 1, latex: false });
    d.acrescentar(Conteudo::Paragrafo { texto: analisar("Gráfico de vendas") });
    d.blocos.last_mut().unwrap().atrib.legenda = 1;
    assert_eq!(sem_ids(&ida_e_volta(&d)), sem_ids(&d));
}

#[test]
fn volta_igual_imagens() {
    let mut d = Documento::vazio();
    let png = crate::base64::para_data_url("image/png", &[0x89, b'P', b'N', b'G', 1, 2, 3]);
    d.acrescentar(Conteudo::Imagem { origem: png, legenda: analisar("A *legenda*"), largura: 120.0, altura: 80.0 });
    d.acrescentar(Conteudo::Imagem { origem: "https://x.org/a.jpg".into(), legenda: Vec::new(), largura: 50.0, altura: 50.0 });
    assert_eq!(sem_ids(&ida_e_volta(&d)), sem_ids(&d));
}

#[test]
fn volta_igual_pagina_estilos_e_cabecalho() {
    use featherdocs_core::modelo::{Faixas, NEGRITO};
    let mut d = Documento::vazio();
    d.pagina = ConfigPagina { largura: 841.89, altura: 595.28, margem_sup: 50.0, margem_dir: 40.0, margem_inf: 60.0, margem_esq: 70.0 };
    d.extras = Extras {
        cabecalho: Faixas { esquerda: "{titulo}".into(), centro: String::new(), direita: "{autor} & cia".into(), ..Faixas::default() },
        rodape: Faixas { esquerda: String::new(), centro: "Página {pagina} de {total}".into(), direita: "{pagina_romano}".into(), ..Faixas::default() },
        primeira_diferente: true,
        cor_pagina: 0xFFF8_E7FF,
        borda_cor: 0x0000_80FF,
        borda_espessura: 1.5,
        borda_distancia: 24.0,
        vertical: Vertical::Centro,
        numerar_linhas: NumeracaoDeLinhas::PorPagina,
        ..Extras::default()
    };
    d.estilos.numeracao = Numeracao::Capitulo;
    d.estilos.lista.push(Estilo { id: 3, nome: "Capa Grande".into(), base: 1, bits: NEGRITO | SERIFA, tamanho: 30.0, cor: 0x3366_99FF, alinhamento: Alinhamento::Centro, antes: Some(0.0), depois: Some(24.0), entrelinha: 1.0, ..Estilo::default() });
    d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("Relatório") });
    d.blocos[0].atrib.estilo = 3;
    d.acrescentar(Conteudo::Titulo { nivel: 2, texto: analisar("Sem número") });
    d.blocos[1].atrib.sem_numero = true;
    let volta = ida_e_volta(&d);
    assert_eq!(volta.pagina, d.pagina);
    assert_eq!(volta.extras, d.extras);
    assert_eq!(volta.estilos, d.estilos);
    assert_eq!(sem_ids(&volta), sem_ids(&d));
}

#[test]
fn rodape_vazio_continua_vazio_e_documento_vazio_grava() {
    let mut d = Documento::default();
    d.extras.rodape = Faixas::default();
    let volta = ida_e_volta(&d);
    assert!(volta.extras.rodape.vazia());
    assert_eq!(sem_ids(&volta), sem_ids(&d));
}

#[test]
fn gravado_tem_as_partes_que_o_word_exige() {
    let bytes = gravar(&Documento::default()).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
    for p in ["[Content_Types].xml", "_rels/.rels", "word/document.xml", "word/_rels/document.xml.rels", "word/styles.xml", "word/numbering.xml", "docProps/core.xml"] {
        assert!(zip.by_name(p).is_ok(), "falta {p}");
    }
}

#[test]
fn notas_indice_e_campos_vao_e_voltam_pelo_word() {
    use featherdocs_core::modelo::Marca;
    let mut d = Documento::vazio();
    let mut texto = featherdocs_core::inline::analisar("Gato");
    texto.push(Trecho::objeto(Marca::Indice { termo: "gato".into(), sub: "felino".into() }));
    texto.push(Trecho::objeto(Marca::Nota { fim: false, texto: featherdocs_core::inline::analisar("Um **bicho**.") }));
    texto.push(Trecho::simples(" por "));
    texto.push(Trecho::objeto(Marca::Campo { campo: "autor".into() }));
    texto.push(Trecho::objeto(Marca::Nota { fim: true, texto: featherdocs_core::inline::analisar("Do fim.") }));
    texto.push(Trecho::objeto(Marca::Citacao { chaves: vec!["silva2023".into()], pagina: "4".into() }));
    d.acrescentar(Conteudo::Paragrafo { texto: texto.clone() });
    let bytes = gravar(&d).unwrap();
    let volta = ler(&bytes).unwrap();
    let Conteudo::Paragrafo { texto: t2 } = &volta.blocos[0].conteudo else { panic!("{:?}", volta.blocos) };
    let marcas = |t: &[Trecho]| t.iter().filter_map(|x| x.marca.as_deref().cloned()).collect::<Vec<_>>();
    assert_eq!(marcas(t2), marcas(&texto));
    assert_eq!(featherdocs_core::inline::texto_puro(t2), "Gato por ");
}

#[test]
fn bibliografia_indice_e_fontes_voltam_pelo_word() {
    use featherdocs_core::modelo::{Autor, EstiloDeCitacao, Fonte, Marca};
    let mut d = Documento::vazio();
    d.bibliografia.estilo = EstiloDeCitacao::Apa;
    d.bibliografia.fontes.push(Fonte { chave: "silva2023".into(), autores: vec![Autor { sobrenome: "Silva".into(), nome: "Ana".into() }], titulo: "Livro".into(), ano: "2023".into(), editora: "Blucher".into(), ..Fonte::default() });
    let mut texto = analisar("Como diz ");
    texto.push(Trecho::objeto(Marca::Citacao { chaves: vec!["silva2023".into()], pagina: String::new() }));
    d.acrescentar(Conteudo::Paragrafo { texto });
    d.acrescentar(Conteudo::Bibliografia { todas: false });
    d.acrescentar(Conteudo::IndiceRemissivo);
    d.acrescentar(Conteudo::Paragrafo { texto: analisar("fim") });
    let volta = ler(&gravar(&d).unwrap()).unwrap();
    assert_eq!(volta.bibliografia, d.bibliografia);
    let tipos: Vec<String> = volta.blocos.iter().map(|b| nome_do_tipo(&b.conteudo)).collect();
    assert_eq!(tipos, ["paragrafo", "bibliografia", "indice_remissivo", "paragrafo"]);
}

#[test]
fn sugestoes_comentarios_e_protecao_voltam_pelo_word() {
    use featherdocs_core::modelo::{Comentario, Marca, Resposta, Revisao, TipoDeRevisao};
    let mut d = Documento::vazio();
    d.extras.controlar_alteracoes = true;
    d.extras.protegido = true;
    let rev = |tipo| Some(Box::new(Revisao { tipo, autor: "Ana".into(), quando: "06/10/2026".into(), movido: false, antes: None }));
    let texto = vec![
        Trecho::simples("O "),
        Trecho::objeto(Marca::Comentario { id: 3, fim: false }),
        Trecho { texto: "gato".into(), rev: rev(TipoDeRevisao::Insercao), ..Trecho::default() },
        Trecho { texto: "cão".into(), rev: rev(TipoDeRevisao::Remocao), ..Trecho::default() },
        Trecho::objeto(Marca::Comentario { id: 3, fim: true }),
        Trecho::simples(" dorme."),
    ];
    d.acrescentar(Conteudo::Paragrafo { texto: texto.clone() });
    d.comentarios.push(Comentario { trechos: Vec::new(), iniciais: String::new(), id: 3, autor: "Ana".into(), quando: "06/10/2026".into(), texto: "Gato ou cão?".into(), resolvido: false, respostas: vec![Resposta { autor: "Bia".into(), quando: "07/10/2026".into(), texto: "Gato.".into() }] });
    let volta = ler(&gravar(&d).unwrap()).unwrap();
    assert_eq!(volta.comentarios, d.comentarios);
    assert!(volta.extras.controlar_alteracoes && volta.extras.protegido);
    let Conteudo::Paragrafo { texto: t2 } = &volta.blocos[0].conteudo else { panic!() };
    assert_eq!(t2, &texto);
}

#[test]
fn volta_igual_quebras_de_secao_coluna_e_linha() {
    use featherdocs_core::modelo::{InicioDeSecao, QUEBRA_DE_LINHA};
    let mut d = Documento::vazio();
    d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples(&format!("linha 1{QUEBRA_DE_LINHA}linha 2"))] });
    d.acrescentar(Conteudo::QuebraDeColuna);
    for inicio in [InicioDeSecao::ProximaPagina, InicioDeSecao::Continua, InicioDeSecao::PaginaPar, InicioDeSecao::PaginaImpar] {
        d.acrescentar(Conteudo::QuebraDeSecao { inicio });
    }
    d.acrescentar(Conteudo::Paragrafo { texto: analisar("fim") });
    assert_eq!(sem_ids(&ida_e_volta(&d)), sem_ids(&d));
}

/// A lista do FeatherDocs volta como lista do FeatherDocs (sem lista do Word): o reinício no meio, o
/// número inicial, o parágrafo recuado que não a interrompe e a lista nova depois de um parágrafo.
#[test]
fn reinicio_e_inicio_da_lista_do_featherdocs_voltam() {
    let mut d = Documento::vazio();
    for t in ["um", "dois", "de novo", "cinco"] {
        d.acrescentar(Conteudo::Item { ordenado: true, nivel: 0, texto: analisar(t) });
    }
    d.blocos[2].atrib.reiniciar = true;
    d.blocos[3].atrib.mais.inicio = Some(5);
    d.acrescentar(Conteudo::Paragrafo { texto: analisar("continua o item") });
    d.blocos[4].atrib.recuo_esq = 36.0;
    d.acrescentar(Conteudo::Item { ordenado: true, nivel: 0, texto: analisar("seis") });
    d.acrescentar(Conteudo::Paragrafo { texto: analisar("fim") });
    d.acrescentar(Conteudo::Item { ordenado: true, nivel: 0, texto: analisar("outra") });
    let volta = ida_e_volta(&d);
    assert!(volta.estilos.listas.is_empty());
    assert_eq!(sem_ids(&volta), sem_ids(&d));
    let m = |d: &Documento| featherdocs_core::paginacao::marcadores(&d.blocos, &d.estilos.listas).into_iter().map(Option::unwrap_or_default).collect::<Vec<_>>();
    assert_eq!(m(&volta), ["1.", "2.", "1.", "5.", "", "6.", "", "1."]);
}

#[test]
fn recuo_do_item_soma_ao_do_nivel_e_volta() {
    let mut d = Documento::vazio();
    d.acrescentar(Conteudo::Item { ordenado: true, nivel: 0, texto: analisar("Um") });
    d.acrescentar(Conteudo::Item { ordenado: false, nivel: 1, texto: analisar("Dois") });
    d.blocos[0].atrib.recuo_esq = 9.5;
    let volta = ida_e_volta(&d);
    // O Word recua 36 pt por nível; o que passa disso é o recuo do item.
    assert_eq!(volta.blocos.iter().map(|b| b.atrib.recuo_esq).collect::<Vec<_>>(), [9.5, 0.0]);
}

/// O gráfico e o controle de conteúdo vão para a sacola e voltam iguais ao `.docx`, passando pelo `.fdoz`:
/// o XML, a parte do gráfico com as relações dela e o `.xlsx` que ela aponta.
#[test]
fn sacola_vai_e_volta_pelo_fdoz() {
    let c = "xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\"";
    let corpo = format!(
        "<w:document {W} {c}><w:body><w:p><w:r><w:t>antes</w:t></w:r><w:r><w:drawing><wp:inline><wp:extent cx=\"1\" cy=\"1\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart r:id=\"rId5\"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>\
         <w:sdt><w:sdtPr><w:alias w:val=\"Nome\"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Ana &amp; Bia</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"
    );
    let grafico = "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:externalData r:id=\"rId1\"/></c:chartSpace>";
    let entrada = docx(&[
        ("[Content_Types].xml", "<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/charts/chart1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawingml.chart+xml\"/></Types>"),
        ("word/document.xml", &corpo),
        ("word/_rels/document.xml.rels", "<Relationships><Relationship Id=\"rId5\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart\" Target=\"charts/chart1.xml\"/></Relationships>"),
        ("word/charts/chart1.xml", grafico),
        ("word/charts/_rels/chart1.xml.rels", "<Relationships><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/package\" Target=\"../embeddings/dados.xlsx\"/></Relationships>"),
        ("word/embeddings/dados.xlsx", "PK planilha"),
    ]);
    let lido = ler(&entrada).unwrap();
    let doc = crate::fdoz::ler(&crate::fdoz::gravar(&lido).unwrap()).unwrap();
    assert_eq!(doc, lido);
    let marcas: Vec<&Marca> = doc.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().filter_map(|t| t.marca.as_deref()).collect();
    let Some(Marca::ObjetoWord { resumo, partes, xml, .. }) = marcas.first() else { panic!("{marcas:?}") };
    assert_eq!(resumo, "gráfico");
    assert!(xml.starts_with("<w:drawing xmlns:") && xml.contains("xmlns:c=") && xml.contains("<c:chart r:id=\"rId5\"/>"), "{xml}");
    assert_eq!(partes.iter().map(|p| p.caminho.as_str()).collect::<Vec<_>>(), ["word/charts/chart1.xml", "word/charts/_rels/chart1.xml.rels", "word/embeddings/dados.xlsx"]);
    assert!(matches!(marcas[1], Marca::Controle { fim: false, bloco: true, xml } if xml.contains("<w:alias w:val=\"Nome\"/>")));
    assert!(matches!(marcas[2], Marca::Controle { fim: true, bloco: true, .. }));
    assert!(doc.blocos.iter().any(|b| texto_puro(b.conteudo.textos()[0]).contains("Ana & Bia")));

    let saida = gravar(&doc).unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(saida.as_slice())).unwrap();
    let mut ler_parte = |n: &str| String::from_utf8(parte(&mut z, n).unwrap_or_default()).unwrap();
    let documento = ler_parte("word/document.xml");
    let rels = ler_parte("word/_rels/document.xml.rels");
    let id = rels.split("Id=\"").find(|r| r.contains("charts/chart1.xml")).and_then(|r| r.split('"').next()).unwrap().to_string();
    assert!(documento.contains(&format!("<c:chart r:id=\"{id}\"/>")), "{documento}");
    assert!(documento.contains("<w:sdt><w:sdtPr") && documento.contains("</w:sdtContent></w:sdt>"), "{documento}");
    assert_eq!(ler_parte("word/charts/chart1.xml"), grafico);
    assert_eq!(ler_parte("word/embeddings/dados.xlsx"), "PK planilha");
    assert!(ler_parte("[Content_Types].xml").contains("PartName=\"/word/charts/chart1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawingml.chart+xml\""));
    // Lido e gravado de novo, o `.docx` sai igual (os `r:id` já são os do FeatherDocs).
    let de_novo = gravar(&ler(&saida).unwrap()).unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(de_novo.as_slice())).unwrap();
    assert_eq!(String::from_utf8(parte(&mut z, "word/document.xml").unwrap()).unwrap(), documento);
}

/// O LaTeX e o Markdown não têm onde pôr o objeto do Word: fica um comentário no lugar, e o texto segue.
#[test]
fn objeto_do_word_vira_comentario_no_latex_e_no_markdown() {
    let mut doc = Documento::vazio();
    let objeto = Trecho::objeto(Marca::ObjetoWord { blocos: Vec::new(), xml: "<w:drawing/>".into(), partes: Vec::new(), resumo: "gráfico".into() });
    doc.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("antes "), objeto, Trecho::simples(" depois")] });
    let tex = crate::exportar::latex(&doc);
    assert!(tex.contains("antes %\n% gráfico do Word: não exportado\n depois"), "{tex}");
    let md = crate::exportar::markdown(&doc);
    assert!(md.contains("antes <!-- gráfico do Word: não exportado --> depois"), "{md}");
}

/// Estilos do Word: o padrão (`docDefaults`), o Normal modificado, o título com cor e a fonte do
/// tema, a herança, o estilo de caractere e o nome da fonte voltam iguais pelo `.docx` e pelo `.fdoz`.
#[test]
fn estilos_do_word_vao_e_voltam() {
    let tema = "<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:themeElements><a:fontScheme name=\"x\"><a:majorFont><a:latin typeface=\"Cambria\"/></a:majorFont><a:minorFont><a:latin typeface=\"Calibri\"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>";
    let estilos = format!(
        "<w:styles {W}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:asciiTheme=\"minorHAnsi\" w:hAnsiTheme=\"minorHAnsi\"/><w:sz w:val=\"22\"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after=\"160\" w:line=\"259\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:pPr><w:jc w:val=\"both\"/></w:pPr></w:style>\
<w:style w:type=\"character\" w:default=\"1\" w:styleId=\"Fontepargpadro\"><w:name w:val=\"Default Paragraph Font\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Ttulo1\"><w:name w:val=\"heading 1\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"0\"/></w:pPr><w:rPr><w:rFonts w:asciiTheme=\"majorHAnsi\" w:hAnsiTheme=\"majorHAnsi\"/><w:color w:val=\"2F5496\"/><w:sz w:val=\"32\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Pai\"><w:name w:val=\"Pai\"/><w:basedOn w:val=\"Normal\"/><w:rPr><w:b/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Filho\"><w:name w:val=\"Filho\"/><w:basedOn w:val=\"Pai\"/><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:rPr><w:color w:val=\"00B050\"/></w:rPr></w:style>\
<w:style w:type=\"character\" w:styleId=\"Enfase\"><w:name w:val=\"Ênfase\"/><w:basedOn w:val=\"Fontepargpadro\"/><w:rPr><w:i/></w:rPr></w:style></w:styles>"
    );
    let corpo = "<w:p><w:pPr><w:pStyle w:val=\"Ttulo1\"/></w:pPr><w:r><w:t>Título</w:t></w:r></w:p>\
<w:p><w:r><w:t xml:space=\"preserve\">Normal </w:t></w:r><w:r><w:rPr><w:rStyle w:val=\"Enfase\"/><w:rFonts w:ascii=\"Georgia\" w:hAnsi=\"Georgia\"/></w:rPr><w:t>ênfase</w:t></w:r></w:p>\
<w:p><w:pPr><w:pStyle w:val=\"Filho\"/><w:jc w:val=\"left\"/></w:pPr><w:r><w:t>filho à esquerda</w:t></w:r></w:p>";
    let d = ler(&docx(&[("word/document.xml", &format!("<w:document {W}><w:body>{corpo}</w:body></w:document>")), ("word/styles.xml", &estilos), ("word/theme/theme1.xml", tema)])).unwrap();
    let nomes: Vec<&str> = d.estilos.lista.iter().map(|e| e.codigo.as_str()).collect();
    assert_eq!(nomes, ["Normal", "Heading1", "Pai", "Filho", "Enfase"], "o Default Paragraph Font não entra; o Ttulo1 vira Heading1");
    assert_eq!((d.estilos.padrao.familia.nome(), d.estilos.padrao.tamanho, d.estilos.padrao.depois), ("+minorHAnsi", 11.0, Some(8.0)));
    let titulo = d.estilos.resolvido_do_bloco(&d.blocos[0]).unwrap();
    assert_eq!((titulo.cor, titulo.tamanho, titulo.bits & SERIFA, titulo.alinhamento), (0x2F54_96FF, 16.0, SERIFA, Alinhamento::Justificado), "Cambria do tema; o justificado vem do Normal");
    assert_eq!(d.blocos[1].atrib.alinhamento, Alinhamento::Justificado, "o Normal carimba o parágrafo sem estilo");
    let t = &d.blocos[1].conteudo.textos()[0][1];
    assert_eq!((t.estilo, t.fmt.familia.nome()), (ITALICO | SERIFA, "Georgia"));
    assert_eq!(d.estilos.pegar(t.fmt.estilo_de_caractere).map(|e| e.nome.as_str()), Some("Ênfase"));
    let filho = d.estilos.do_bloco(&d.blocos[2]).unwrap();
    assert_eq!(d.estilos.pegar(filho.herda).map(|e| e.nome.as_str()), Some("Pai"));
    assert_eq!(d.blocos[2].atrib.alinhamento, Alinhamento::Esquerda, "o w:jc direto ganha do estilo");
    let volta = ida_e_volta(&d);
    assert_eq!(volta.estilos, d.estilos);
    assert_eq!(sem_ids(&volta), sem_ids(&d));
    let pelo_fdoz = crate::fdoz::ler(&crate::fdoz::gravar(&d).unwrap()).unwrap();
    assert_eq!(pelo_fdoz.estilos, d.estilos);
    assert_eq!(sem_ids(&pelo_fdoz), sem_ids(&d));
}

/// O estilo filho que desliga o que o pai liga (`<w:b w:val="0"/>`, o `w:jc` à esquerda, o
/// `w:keepNext` desligado) vence o pai e o padrão, e volta igual pelo `.docx` e pelo `.fdoz`.
#[test]
fn estilo_filho_desliga_o_do_pai_e_volta() {
    use featherdocs_core::modelo::{MANTER_COM_PROXIMO, VERSALETE};
    let estilos = format!(
        "<w:styles {W}><w:docDefaults><w:rPrDefault><w:rPr><w:b/></w:rPr></w:rPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:styleId=\"Pai\"><w:name w:val=\"Pai\"/><w:pPr><w:keepNext/><w:jc w:val=\"center\"/></w:pPr><w:rPr><w:i/><w:smallCaps/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Filho\"><w:name w:val=\"Filho\"/><w:basedOn w:val=\"Pai\"/><w:pPr><w:keepNext w:val=\"0\"/><w:jc w:val=\"left\"/></w:pPr><w:rPr><w:b w:val=\"0\"/><w:smallCaps w:val=\"0\"/></w:rPr></w:style></w:styles>"
    );
    let corpo = "<w:p><w:pPr><w:pStyle w:val=\"Filho\"/></w:pPr><w:r><w:t>filho</w:t></w:r></w:p>";
    let d = ler(&docx(&[("word/document.xml", &format!("<w:document {W}><w:body>{corpo}</w:body></w:document>")), ("word/styles.xml", &estilos)])).unwrap();
    let confere = |d: &Documento| {
        let f = d.estilos.resolvido_do_bloco(&d.blocos[0]).unwrap();
        assert_eq!(f.bits & (NEGRITO | ITALICO), ITALICO, "o negrito do padrão desliga; o itálico do pai fica");
        assert_eq!(f.alinhamento, Alinhamento::Esquerda);
        assert_eq!((f.mais_letra.efeitos & VERSALETE, f.mais_paragrafo.fluxo & MANTER_COM_PROXIMO), (0, 0));
        assert_eq!(d.blocos[0].atrib.alinhamento, Alinhamento::Esquerda, "o parágrafo carimbado pelo filho fica à esquerda");
    };
    confere(&d);
    let xml = crate::docx_gravar::estilos_xml(&d);
    let filho = &xml[xml.find("w:styleId=\"Filho\"").unwrap()..];
    let filho = &filho[..filho.find("</w:style>").unwrap()];
    for tem in ["<w:b w:val=\"0\"/>", "<w:smallCaps w:val=\"0\"/>", "<w:keepNext w:val=\"0\"/>", "<w:jc w:val=\"left\"/>"] {
        assert!(filho.contains(tem), "{tem} em {filho}");
    }
    let volta = ida_e_volta(&d);
    confere(&volta);
    assert_eq!(volta.estilos, d.estilos);
    let pelo_fdoz = crate::fdoz::ler(&crate::fdoz::gravar(&d).unwrap()).unwrap();
    assert_eq!(pelo_fdoz.estilos, d.estilos);
}

/// O parágrafo com `w:outlineLvl` (sem estilo de título) entra na estrutura; o `w:pageBreakBefore` sai
/// como quebra no LaTeX e no Markdown exportados.
#[test]
fn nivel_de_topico_na_estrutura_e_quebra_antes_nos_exportadores() {
    use featherdocs_core::modelo::QUEBRA_ANTES;
    let d = ler(&corpo("<w:p><w:pPr><w:outlineLvl w:val=\"1\"/></w:pPr><w:r><w:t>Tópico</w:t></w:r></w:p><w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:r><w:t>Nova página</w:t></w:r></w:p>")).unwrap();
    assert!(matches!(d.blocos[0].conteudo, Conteudo::Paragrafo { .. }) && d.blocos[0].atrib.mais.nivel_topico == 2);
    assert!(d.blocos[1].atrib.mais.fluxo & QUEBRA_ANTES != 0 && d.blocos.len() == 2, "a quebra fica no parágrafo, sem bloco a mais");
    let s = featherdocs_core::sessao::Sessao::com_documento(d.clone());
    assert_eq!(s.estrutura().iter().map(|t| (t.nivel, t.texto.as_str())).collect::<Vec<_>>(), [(2, "Tópico")]);
    let tex = crate::exportar::latex(&d);
    assert!(tex.contains("\\newpage\n\nNova página"), "{tex}");
    let md = crate::exportar::markdown(&d);
    assert!(md.contains("<div style=\"page-break-after: always\"></div>\n\nNova página"), "{md}");
}

#[test]
fn propriedades_da_tabela_vao_e_voltam_pelo_fdoz_e_pelo_docx() {
    let tc = |pr: &str, t: &str| format!("<w:tc><w:tcPr>{pr}</w:tcPr><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc>");
    let d = ler(&corpo(&format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"2500\" w:type=\"pct\"/><w:jc w:val=\"center\"/><w:tblLayout w:type=\"fixed\"/></w:tblPr>\
         <w:tblGrid><w:gridCol w:w=\"2000\"/><w:gridCol w:w=\"2000\"/><w:gridCol w:w=\"4000\"/></w:tblGrid>\
         <w:tr><w:trPr><w:trHeight w:val=\"600\"/></w:trPr>{}{}</w:tr>\
         <w:tr>{}{}{}</w:tr></w:tbl>",
        tc("<w:gridSpan w:val=\"2\"/><w:shd w:val=\"clear\" w:fill=\"FFC000\"/>", "largo"),
        tc("<w:vMerge w:val=\"restart\"/><w:vAlign w:val=\"center\"/>", "alto"),
        tc("", "1"),
        tc("", "2"),
        tc("<w:vMerge/>", ""),
    )))
    .unwrap();
    let b = &d.blocos[0];
    let Conteudo::Tabela { linhas, .. } = &b.conteudo else { panic!() };
    assert_eq!(linhas.iter().map(Vec::len).collect::<Vec<_>>(), vec![3, 3], "a célula encoberta pelo gridSpan fica, vazia");
    assert_eq!(b.mesclagens, vec![MesclagemTabela { linha: 0, coluna: 0, colunas: 2 }]);
    assert_eq!(b.tabela.colunas, vec![100.0, 100.0, 200.0]);
    assert_eq!((b.tabela.largura_pct, b.tabela.fixa, b.atrib.alinhamento), (50.0, true, Alinhamento::Centro));
    assert_eq!(b.tabela.celula(0, 0).map(|c| c.fundo), Some(0xFFC000FF));
    assert_eq!(b.tabela.celula(0, 2).map(|c| (c.linhas, c.vertical)), Some((2, 1)));
    assert_eq!(b.tabela.altura(0).map(|a| a.altura), Some(30.0));

    let pelo_fdoz = crate::fdoz::ler(&crate::fdoz::gravar(&d).unwrap()).unwrap();
    assert_eq!(pelo_fdoz.blocos[0].tabela, b.tabela);
    let de_volta = ler(&gravar(&pelo_fdoz).unwrap()).unwrap();
    assert_eq!(de_volta.blocos[0].tabela, b.tabela);
    assert_eq!(de_volta.blocos[0].mesclagens, b.mesclagens);
}

/// A célula com dois parágrafos e uma tabela dentro guarda os blocos, mostra o texto deles e volta
/// igual pelo `.docx` e pelo `.fdoz`; editado o texto da célula, sai o texto (sem os blocos velhos).
#[test]
fn celula_com_blocos_volta_e_a_edicao_vence() {
    let cel = |x: &str| format!("<w:tc><w:p><w:r><w:t>{x}</w:t></w:r></w:p></w:tc>");
    let dentro = format!("<w:tbl><w:tr>{}{}</w:tr></w:tbl>", cel("a"), cel("b"));
    let xml = format!("<w:tbl><w:tr><w:tc><w:p><w:r><w:t>um</w:t></w:r></w:p><w:p><w:r><w:t>dois</w:t></w:r></w:p>{dentro}<w:p/></w:tc>{}</w:tr></w:tbl>", cel("só"));
    let d = ler(&corpo(&xml)).unwrap();
    let confere = |d: &Documento| {
        assert_eq!(d.blocos.len(), 1, "a tabela de dentro não sai da célula");
        let b = &d.blocos[0];
        let Conteudo::Tabela { linhas, .. } = &b.conteudo else { panic!() };
        let c = b.tabela.celula(0, 0).unwrap();
        assert_eq!(c.blocos.len(), 4);
        assert!(matches!(&c.blocos[2].conteudo, Conteudo::Tabela { linhas, .. } if texto_puro(&linhas[0][1]) == "b"));
        assert_eq!(texto_puro(&linhas[0][0]), format!("um{q}dois{q}a b{q}", q = QUEBRA_DE_LINHA));
        assert!(c.blocos_valem(&linhas[0][0]));
        assert!(b.tabela.celula(0, 1).is_none(), "a célula de um parágrafo só fica só texto");
    };
    confere(&d);
    confere(&ida_e_volta(&d));
    confere(&crate::fdoz::ler(&crate::fdoz::gravar(&d).unwrap()).unwrap());
    let mut editado = d.clone();
    if let Conteudo::Tabela { linhas, .. } = &mut editado.blocos[0].conteudo {
        linhas[0][0] = vec![Trecho::simples("novo")];
    }
    let volta = ida_e_volta(&editado);
    let Conteudo::Tabela { linhas, .. } = &volta.blocos[0].conteudo else { panic!() };
    assert_eq!(texto_puro(&linhas[0][0]), "novo");
    assert!(volta.blocos[0].tabela.celula(0, 0).is_none_or(|c| c.blocos.is_empty()));
}

/// Fase 5: duas seções (retrato com cabeçalho próprio, depois paisagem), cabeçalho de páginas pares,
/// tabela no rodapé, marca d'água e o resto do `w:sectPr` vão e voltam pelo `.docx` e pelo `.fdoz`.
#[test]
fn secoes_faixas_e_pagina_do_word_voltam() {
    use featherdocs_core::modelo::{Faixas, InicioDeSecao, MaisDocumento, Secao};
    let mut d = Documento::vazio();
    d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("um")] });
    d.acrescentar(Conteudo::QuebraDeSecao { inicio: InicioDeSecao::ProximaPagina });
    d.blocos[1].secao = Some(Box::new(Secao { pagina: ConfigPagina::a4(), cabecalho: Faixas { esquerda: "primeira seção".into(), ..Faixas::default() }, rodape: Faixas::default() }));
    d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("dois")] });
    let a4 = ConfigPagina::a4();
    d.pagina = ConfigPagina { largura: a4.altura, altura: a4.largura, ..a4 };
    d.extras.cabecalho = Faixas { esquerda: "ímpar".into(), estilo: NEGRITO, tamanho: 14.0, cor: 0xC00000FF, ..Faixas::default() };
    d.extras.rodape = Faixas { esquerda: "a b".into(), blocos: vec![Bloco::novo(90, Conteudo::Tabela { cabecalho: false, linhas: vec![vec![vec![Trecho::simples("a")], vec![Trecho::simples("b")]]] })], ..Faixas::default() };
    d.extras.rodape.lida = d.extras.rodape.assinatura();
    (d.extras.marca, d.extras.marca_cor, d.extras.pagina_inicial) = ("RASCUNHO".into(), 0xC0C0C066, 3);
    d.extras.documento = MaisDocumento { medianiz: 28.35, distancia_cabecalho: 50.0, colunas: 2, espaco_colunas: 18.0, formato_pagina: "upperRoman".into(), borda_estilo: String::new(), pares_diferentes: true, cabecalho_par: Faixas { direita: "par".into(), ..Faixas::default() }, ..MaisDocumento::default() };
    let confere = |v: &Documento| {
        assert_eq!(v.pagina, d.pagina);
        assert_eq!(v.blocos[1].secao, d.blocos[1].secao, "a seção do meio");
        let (x, y) = (&v.extras, &d.extras);
        assert_eq!((&x.cabecalho, &x.documento), (&y.cabecalho, &y.documento));
        // O Word pede um parágrafo depois da tabela: ele entra na primeira volta e fica.
        assert!(matches!(&x.rodape.blocos[..], [b, ..] if matches!(&b.conteudo, Conteudo::Tabela { linhas, .. } if texto_puro(&linhas[0][1]) == "b")), "{:?}", x.rodape);
        assert_eq!((&x.marca, x.marca_cor, x.pagina_inicial), (&y.marca, y.marca_cor, y.pagina_inicial));
    };
    let volta = ida_e_volta(&d);
    confere(&volta);
    confere(&crate::fdoz::ler(&crate::fdoz::gravar(&volta).unwrap()).unwrap());
    // Editado o rodapé na tela, os blocos velhos não entram mais.
    let mut editado = volta.clone();
    editado.extras.rodape.esquerda = "novo".into();
    let de_novo = ida_e_volta(&editado);
    assert_eq!(de_novo.extras.rodape.esquerda, "novo");
    assert!(de_novo.extras.rodape.blocos.is_empty());
}

/// Fase 6: a imagem com texto alternativo, recorte, rotação e âncora, a imagem no meio do texto e a
/// caixa de texto (com o texto dela em blocos, lido uma vez só) voltam pelo `.docx` e pelo `.fdoz`.
#[test]
fn imagens_e_caixa_de_texto_do_word_voltam() {
    use featherdocs_core::modelo::{Flutuante, PosicaoFlutuante, PropsImagem, TEXTO_DA_CAIXA};
    let png = crate::base64::para_data_url("image/png", b"\x89PNG\r\n\x1a\nfalso");
    let mut d = Documento::vazio();
    d.acrescentar(Conteudo::Imagem { origem: png.clone(), legenda: Vec::new(), largura: 120.0, altura: 60.0 });
    d.blocos[0].imagem = PropsImagem {
        descricao: "Logo".into(),
        recorte: [10.0, 5.0, 0.0, 2.5],
        rotacao: 90.0,
        espelhar_h: true,
        flutuante: Some(Flutuante { horizontal: PosicaoFlutuante { relativo: "margin".into(), alinhar: "right".into(), deslocamento: 0.0 }, vertical: PosicaoFlutuante { relativo: "paragraph".into(), alinhar: String::new(), deslocamento: 12.0 }, quebra: "tight".into(), lado: "left".into(), atras: false, distancias: [0.0, 0.0, 9.0, 9.0] }),
        ..PropsImagem::default()
    };
    d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("antes "), Trecho::objeto(Marca::Imagem { origem: png, largura: 12.0, altura: 12.0 }), Trecho::simples(" depois")] });
    let confere = |v: &Documento| {
        assert_eq!(v.blocos[0].imagem, d.blocos[0].imagem);
        let Conteudo::Paragrafo { texto } = &v.blocos[1].conteudo else { panic!("{:?}", v.blocos[1].conteudo) };
        assert!(matches!(texto.get(1).and_then(|t| t.marca.as_deref()), Some(Marca::Imagem { largura, .. }) if *largura == 12.0), "{texto:?}");
    };
    let volta = ida_e_volta(&d);
    confere(&volta);
    confere(&crate::fdoz::ler(&crate::fdoz::gravar(&volta).unwrap()).unwrap());

    // A caixa de texto do Word, com a VML antiga no `mc:Fallback`.
    let txbx = "<w:txbxContent><w:p><w:r><w:t>dentro um</w:t></w:r></w:p><w:p><w:r><w:t>dentro dois</w:t></w:r></w:p></w:txbxContent>";
    let xml = format!("<w:p><w:r><mc:AlternateContent xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\"><mc:Choice Requires=\"wps\"><w:drawing><wp:inline xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\"><wp:extent cx=\"1828800\" cy=\"457200\"/><wp:docPr id=\"5\" name=\"Caixa 5\"/><a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:graphicData uri=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"><wps:wsp xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"><wps:spPr/><wps:txbx>{txbx}</wps:txbx><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:rect xmlns:v=\"urn:schemas-microsoft-com:vml\"><v:textbox>{txbx}</v:textbox></v:rect></w:pict></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t>fora</w:t></w:r></w:p>");
    let d = ler(&corpo(&xml)).unwrap();
    let caixa = |d: &Documento| -> (String, Vec<String>, String) {
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!("{:?}", d.blocos) };
        let Some(Marca::ObjetoWord { xml, blocos, resumo, .. }) = texto[0].marca.as_deref() else { panic!("{texto:?}") };
        assert!(xml.contains(TEXTO_DA_CAIXA) && !xml.contains("dentro"), "o texto sai do XML guardado: {xml}");
        (resumo.clone(), blocos.iter().flat_map(|b| b.conteudo.textos()).map(|t| texto_puro(t)).collect(), texto_puro(texto))
    };
    let esperado = ("caixa de texto".to_string(), vec!["dentro um".to_string(), "dentro dois".to_string()], "fora".to_string());
    assert_eq!(d.blocos.len(), 1, "o texto da caixa não vira parágrafo solto: {:?}", d.blocos);
    assert_eq!(caixa(&d), esperado);
    let volta = ida_e_volta(&d);
    assert_eq!(caixa(&volta), esperado);
    assert_eq!(caixa(&crate::fdoz::ler(&crate::fdoz::gravar(&volta).unwrap()).unwrap()), esperado);
}

/// Fase 7: campos guardados (MERGEFIELD, REF) e a data com formato, o indicador, a nota com dois
/// parágrafos e o formato das notas, e o texto movido voltam pelo `.docx` e pelo `.fdoz`.
#[test]
fn campos_indicadores_notas_e_movidos_voltam() {
    use featherdocs_core::campo;
    use featherdocs_core::modelo::{Marcador, QUEBRA_DE_PARAGRAFO};
    let mut d = Documento::vazio();
    let rev = |tipo| Some(Box::new(Revisao { tipo, autor: "Ana".into(), quando: "06/10/2026".into(), movido: true, antes: None }));
    let id = d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("Alvo"), Trecho { rev: rev(TipoDeRevisao::Remocao), ..Trecho::simples(" saiu") }] });
    d.marcadores.push(Marcador { nome: "sec1".into(), bloco: id });
    d.acrescentar(Conteudo::Paragrafo { texto: vec![
        Trecho::objeto(campo::do_word("MERGEFIELD Nome", "«Nome»")),
        Trecho::simples(" "),
        Trecho::objeto(campo::do_word("REF sec1 \\h", "Alvo")),
        Trecho::objeto(Marca::Campo { campo: campo::data_com_formato("d 'de' MMMM") }),
        Trecho::objeto(Marca::Nota { fim: false, texto: vec![Trecho::simples(&format!("um{QUEBRA_DE_PARAGRAFO}dois"))] }),
        Trecho { rev: rev(TipoDeRevisao::Insercao), ..Trecho::simples(" saiu") },
    ] });
    (d.extras.formato_notas, d.extras.documento.notas_por_pagina) = (2, true);
    let confere = |v: &Documento| {
        assert_eq!(v.marcadores.iter().map(|m| (m.nome.as_str(), v.blocos.iter().position(|b| b.id == m.bloco))).collect::<Vec<_>>(), [("sec1", Some(0))]);
        let Conteudo::Paragrafo { texto } = &v.blocos[1].conteudo else { panic!() };
        let marcas: Vec<&Marca> = texto.iter().filter_map(|t| t.marca.as_deref()).collect();
        let campos: Vec<&str> = marcas.iter().filter_map(|m| if let Marca::Campo { campo } = m { Some(campo.as_str()) } else { None }).collect();
        assert_eq!(campos.len(), 3, "{campos:?}");
        assert_eq!(campo::partes_do_word(campos[0]), Some(("MERGEFIELD Nome", "«Nome»")));
        assert_eq!(campo::partes_do_word(campos[1]), Some(("REF sec1 \\h", "Alvo")));
        assert_eq!(campo::formato_da_data(campos[2]), Some("d 'de' MMMM"));
        assert!(marcas.iter().any(|m| matches!(m, Marca::Nota { texto, .. } if texto_puro(texto) == format!("um{QUEBRA_DE_PARAGRAFO}dois"))), "{marcas:?}");
        let movidos: Vec<TipoDeRevisao> = v.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().filter_map(|t| t.rev.as_deref()).filter(|r| r.movido).map(|r| r.tipo).collect();
        assert_eq!(movidos, [TipoDeRevisao::Remocao, TipoDeRevisao::Insercao]);
        assert_eq!((v.extras.formato_notas, v.extras.documento.notas_por_pagina), (2, true));
    };
    let volta = ida_e_volta(&d);
    confere(&volta);
    confere(&crate::fdoz::ler(&crate::fdoz::gravar(&volta).unwrap()).unwrap());
}

/// Fase 8: as propriedades do arquivo (core, app, custom), o idioma do trecho e o do documento, a
/// direção do texto, a hifenização e a tabulação padrão voltam pelo `.docx` e pelo `.fdoz`; o
/// português que o gravador põe sozinho não vira idioma do documento na volta.
#[test]
fn propriedades_idioma_e_direcao_voltam() {
    use featherdocs_core::modelo::{Familia, MaisDocumento, Meta, DIREITA_PARA_ESQUERDA, SEM_HIFENIZAR};
    let mut d = Documento::vazio();
    d.meta = Meta { titulo: "T".into(), autor: "Ana".into() };
    d.extras.documento = MaisDocumento { assunto: "Vendas".into(), palavras_chave: "anual".into(), descricao: "d".into(), criado: "2026-10-28T10:00:00Z".into(), modificado: "2026-10-29T10:00:00Z".into(), empresa: "Feather".into(), gerente: "Bia".into(), personalizadas: vec![("Projeto".into(), "Pena & cia".into())], hifenizar: true, tabulacao: 56.7, idioma: "es-ES".into(), ..Default::default() };
    let mut t = Trecho::simples("olá");
    (t.fmt.mais.outros, t.fmt.mais.idioma) = (DIREITA_PARA_ESQUERDA, Familia::de("he-IL"));
    d.acrescentar(Conteudo::Paragrafo { texto: vec![t] });
    d.blocos[0].atrib.mais.mais_fluxo = DIREITA_PARA_ESQUERDA | SEM_HIFENIZAR;
    let confere = |v: &Documento| {
        assert_eq!(v.meta, d.meta);
        assert_eq!(v.blocos[0].atrib.mais.mais_fluxo, d.blocos[0].atrib.mais.mais_fluxo);
        assert_eq!(v.blocos[0].conteudo, d.blocos[0].conteudo);
        assert_eq!(v.extras.documento, d.extras.documento);
        assert!(v.estilos.padrao.mais_letra.idioma.vazia(), "o idioma padrão fica no documento, não no estilo");
    };
    let volta = ida_e_volta(&d);
    confere(&volta);
    confere(&crate::fdoz::ler(&crate::fdoz::gravar(&volta).unwrap()).unwrap());
}

#[test]
fn decoracao_presa_a_pagina_vai_e_volta_pelo_docx_e_pelo_fdoz() {
    use featherdocs_core::modelo::Decoracao;
    let mut d = Documento::vazio();
    let id = d.acrescentar(Conteudo::Paragrafo { texto: analisar("Capa") });
    d.acrescentar(Conteudo::Paragrafo { texto: analisar("Resto") });
    let svg = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHdpZHRoPSc0JyBoZWlnaHQ9JzQnLz4=";
    d.extras.decoracoes.push(Decoracao { origem: svg.into(), x: 400.0, y: 0.0, largura: 195.25, altura: 120.0, atras: true, bloco: Some(id) });
    let lido = ler(&gravar(&d).unwrap()).unwrap();
    let [dec] = lido.extras.decoracoes.as_slice() else { panic!("{:?}", lido.extras.decoracoes) };
    assert_eq!((dec.x, dec.y, dec.largura, dec.altura, dec.atras), (400.0, 0.0, 195.25, 120.0, true));
    let capa = lido.blocos.iter().find(|b| matches!(&b.conteudo, Conteudo::Paragrafo { texto } if texto_puro(texto) == "Capa")).unwrap();
    assert_eq!(dec.bloco, Some(capa.id), "a âncora fica no bloco da capa");
    assert!(lido.blocos.iter().all(|b| !matches!(b.conteudo, Conteudo::Imagem { .. })), "não vira imagem em linha");
    let pelo_fdoz = crate::fdoz::ler(&crate::fdoz::gravar(&lido).unwrap()).unwrap();
    assert_eq!(pelo_fdoz.extras.decoracoes, lido.extras.decoracoes);
}

#[test]
fn papel_timbrado_vai_e_volta_pelo_docx() {
    for id in ["timbrado", "timbrado-elegante", "comunicado"] {
        let mut d = featherdocs_core::modelos::criar(id).unwrap();
        d.extras.marca = "RASCUNHO".into();
        d.extras.primeira_diferente = true;
        d.extras.cabecalho_primeira = featherdocs_core::modelo::Faixas { centro: "Só na capa".into(), ..Default::default() };
        let volta = ler(&crate::docx_gravar::gravar(&d).unwrap()).unwrap();
        let (a, b) = (&d.extras, &volta.extras);
        assert_eq!((b.cabecalho.estilo, b.cabecalho.tamanho, b.cabecalho.cor), (a.cabecalho.estilo, a.cabecalho.tamanho, a.cabecalho.cor), "{id}: a letra do timbre");
        assert_eq!((&b.cabecalho.esquerda, &b.rodape.esquerda, &b.rodape.centro), (&a.cabecalho.esquerda, &a.rodape.esquerda, &a.rodape.centro), "{id}");
        assert_eq!(b.marca, "RASCUNHO", "{id}: a marca d'água");
        assert_eq!(b.cabecalho_primeira.centro, "Só na capa", "{id}: a faixa da primeira página");
        let toda_pagina = |x: &featherdocs_core::modelo::Extras| x.decoracoes.iter().filter(|d| d.bloco.is_none()).map(|d| (d.x.round(), d.y.round(), d.largura.round(), d.atras)).collect::<Vec<_>>();
        assert_eq!(toda_pagina(b), toda_pagina(a), "{id}: o timbre fica em toda página, na mesma posição e ordem");
    }
}

/// Fase 9: a formatação e o parágrafo sugeridos, a marca de parágrafo e a linha inseridas, a dica do
/// link, o negrito no comentário, as entradas do sumário e a fonte embutida voltam pelo `.docx` e
/// pelo `.fdoz`; rejeitar a formatação sugerida volta à de antes.
#[test]
fn sugestoes_do_bloco_dica_comentario_sumario_e_fonte_voltam() {
    use featherdocs_core::modelo::{Comentario, Familia, FonteEmbutida, Fmt, Marca, Revisao, RevisoesDoBloco, TipoDeRevisao};
    let rev = |tipo| Revisao { tipo, autor: "Ana".into(), quando: "28/10/2026".into(), movido: false, antes: None };
    let mut d = Documento::vazio();
    d.acrescentar(Conteudo::sumario(3));
    d.extras.documento.entradas_sumario = vec![(1, "Introdução".into(), "1".into()), (2, "Método".into(), "2".into())];
    let mut link = Trecho { link: Some("https://example.com".into()), ..Trecho::simples("site") };
    link.fmt.mais.dica = Familia::de("Abrir o site");
    let negrito = Trecho { estilo: NEGRITO, rev: Some(Box::new(Revisao { antes: Some(Box::new((0, Fmt::default()))), ..rev(TipoDeRevisao::Formatacao) })), ..Trecho::simples("forte") };
    let id = d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::objeto(Marca::Comentario { id: 1, fim: false }), negrito, Trecho::simples(" e "), link, Trecho::objeto(Marca::Comentario { id: 1, fim: true })] });
    let b = d.blocos.iter_mut().find(|b| b.id == id).unwrap();
    b.atrib.alinhamento = Alinhamento::Centro;
    b.revisao = Some(Box::new(RevisoesDoBloco { paragrafo: Some((rev(TipoDeRevisao::Formatacao), featherdocs_core::modelo::Atrib::default())), marca: Some(rev(TipoDeRevisao::Insercao)), linhas: Vec::new() }));
    let t = d.acrescentar(Conteudo::Tabela { cabecalho: false, linhas: vec![vec![vec![Trecho::simples("a")]], vec![vec![Trecho::simples("b")]]] });
    d.blocos.iter_mut().find(|b| b.id == t).unwrap().revisao = Some(Box::new(RevisoesDoBloco { linhas: vec![(1, rev(TipoDeRevisao::Insercao))], ..Default::default() }));
    d.comentarios.push(Comentario { id: 1, autor: "Bia".into(), texto: "muito bom".into(), trechos: vec![Trecho { estilo: NEGRITO, ..Trecho::simples("muito") }, Trecho::simples(" bom")], ..Default::default() });
    d.extras.documento.fontes_embutidas = vec![FonteEmbutida { nome: "Minha Fonte".into(), variante: "embedRegular".into(), chave: "{00000000-0000-0000-0000-000000000001}".into(), subconjunto: false, bytes: vec![1, 2, 3] }];
    let confere = |v: &Documento| {
        let x = &v.extras.documento;
        assert_eq!((&x.entradas_sumario, &x.fontes_embutidas), (&d.extras.documento.entradas_sumario, &d.extras.documento.fontes_embutidas));
        let p = v.blocos.iter().find(|b| matches!(&b.conteudo, Conteudo::Paragrafo { texto } if texto_puro(texto).contains("forte"))).expect("o parágrafo");
        assert_eq!(p.revisao, d.blocos[1].revisao, "a sugestão do parágrafo e a marca");
        let Conteudo::Paragrafo { texto } = &p.conteudo else { unreachable!() };
        let forte = texto.iter().find(|t| t.texto == "forte").unwrap();
        assert_eq!(forte.rev.as_deref().map(|r| (r.tipo, r.antes.as_deref().map(|a| a.0))), Some((TipoDeRevisao::Formatacao, Some(0))));
        assert_eq!(texto.iter().find(|t| t.link.is_some()).map(|t| t.fmt.mais.dica.nome()), Some("Abrir o site"));
        let tabela = v.blocos.iter().find(|b| matches!(b.conteudo, Conteudo::Tabela { .. })).unwrap();
        assert_eq!(tabela.revisao.as_deref().map(|r| r.linhas.iter().map(|(i, r)| (*i, r.tipo)).collect::<Vec<_>>()), Some(vec![(1, TipoDeRevisao::Insercao)]));
        assert_eq!(v.comentarios[0].trechos_validos(), Some(d.comentarios[0].trechos.as_slice()));
    };
    let volta = ida_e_volta(&d);
    confere(&volta);
    confere(&crate::fdoz::ler(&crate::fdoz::gravar(&volta).unwrap()).unwrap());

    // Rejeitar tudo: o negrito sai, o parágrafo volta ao de antes e a linha inserida some.
    let mut s = featherdocs_core::sessao::Sessao::com_documento(volta);
    s.revisoes_resolver(false, true);
    let v = s.documento();
    let p = v.blocos.iter().find(|b| matches!(&b.conteudo, Conteudo::Paragrafo { texto } if texto_puro(&texto[..]).contains("forte"))).unwrap();
    let Conteudo::Paragrafo { texto } = &p.conteudo else { unreachable!() };
    assert!(texto.iter().all(|t| t.estilo & NEGRITO == 0 && t.rev.is_none()), "{texto:?}");
    assert!(p.revisao.is_none() && p.atrib.alinhamento != Alinhamento::Centro);
    assert!(matches!(&v.blocos.iter().find(|b| matches!(b.conteudo, Conteudo::Tabela { .. })).unwrap().conteudo, Conteudo::Tabela { linhas, .. } if linhas.len() == 1));
}
