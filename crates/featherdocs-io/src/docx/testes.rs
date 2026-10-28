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
        Trecho { texto: "serifa grande".into(), estilo: SERIFA | ITALICO, fmt: Fmt { tamanho: 20.0, cor: 0xFF00_00FF, realce: 0xFFFF_00FF, posicao: 0 }, ..Trecho::default() },
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
    d.estilos.lista.push(Estilo { id: 3, nome: "Capa Grande".into(), base: 1, bits: NEGRITO | SERIFA, tamanho: 30.0, cor: 0x3366_99FF, alinhamento: Alinhamento::Centro, antes: Some(0.0), depois: Some(24.0), entrelinha: 1.0 });
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
    let rev = |tipo| Some(Box::new(Revisao { tipo, autor: "Ana".into(), quando: "06/10/2026".into() }));
    let texto = vec![
        Trecho::simples("O "),
        Trecho::objeto(Marca::Comentario { id: 3, fim: false }),
        Trecho { texto: "gato".into(), rev: rev(TipoDeRevisao::Insercao), ..Trecho::default() },
        Trecho { texto: "cão".into(), rev: rev(TipoDeRevisao::Remocao), ..Trecho::default() },
        Trecho::objeto(Marca::Comentario { id: 3, fim: true }),
        Trecho::simples(" dorme."),
    ];
    d.acrescentar(Conteudo::Paragrafo { texto: texto.clone() });
    d.comentarios.push(Comentario { id: 3, autor: "Ana".into(), quando: "06/10/2026".into(), texto: "Gato ou cão?".into(), resolvido: false, respostas: vec![Resposta { autor: "Bia".into(), quando: "07/10/2026".into(), texto: "Gato.".into() }] });
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
    let Some(Marca::ObjetoWord { resumo, partes, xml }) = marcas.first() else { panic!("{marcas:?}") };
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
    let objeto = Trecho::objeto(Marca::ObjetoWord { xml: "<w:drawing/>".into(), partes: Vec::new(), resumo: "gráfico".into() });
    doc.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("antes "), objeto, Trecho::simples(" depois")] });
    let tex = crate::exportar::latex(&doc);
    assert!(tex.contains("antes %\n% gráfico do Word: não exportado\n depois"), "{tex}");
    let md = crate::exportar::markdown(&doc);
    assert!(md.contains("antes <!-- gráfico do Word: não exportado --> depois"), "{md}");
}
