//! A árvore de blocos para `.docx` (Office Open XML), o formato que o Word, o
//! LibreOffice e o Google Docs abrem. O `.docx` é um ZIP de XMLs; aqui se monta
//! cada parte como texto, sem árvore DOM.
//!
//! Grava: títulos (estilos "heading N", com a numeração automática ligada aos
//! estilos), parágrafo, citação, código, listas (nove tipos de marcador), tabelas,
//! imagens (`data:` vira arquivo em `word/media/`, endereço da web vira imagem
//! ligada), fórmulas (OMML, a equação nativa do Word, montada pelo `feather-math`), sumário e listas de figuras
//! (campo `TOC`, que o Word atualiza ao abrir), legendas (campo `SEQ`), régua,
//! quebra de página, links, formatação de caractere (negrito, itálico, sublinhado,
//! tachado, fonte, tamanho, cor, realce, sobrescrito/subscrito) e de parágrafo
//! (alinhamento, espaços, entrelinha, recuos, fundo, bordas), estilos com nome,
//! papel e margens, cabeçalho e rodapé com campos, cor e borda da página,
//! alinhamento vertical e números de linha.
//!
//! Os objetos da sacola (forma, gráfico, SmartArt, objeto OLE, controle de conteúdo) saem com o
//! XML original e as partes deles. Não grava (ainda): marca d'água e a linguagem do bloco de código.
//! O leitor (`docx.rs`) entende de volta tudo o que está aqui.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use featherdocs_core::layout::tamanho_base;
use featherdocs_core::modelo::{Alinhamento, Atrib, Bloco, Celula, Conteudo, Documento, EstiloDeCitacao, Estilo, Estilos, Faixas, Fmt, InicioDeSecao, Lista, Marca, Numeracao, NumeracaoDeLinhas, TipoDeRevisao, Trecho, Vertical, MaisLetra, MaisParagrafo, ALINHAMENTO_DEFINIDO, CODIGO, MATEMATICA, SERIFA};
use zip::write::SimpleFileOptions;

use crate::base64;
use crate::docx::mapa;

const NS: &str = concat!(
    "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" ",
    "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
    "xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" ",
    "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" ",
    "xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\""
);
const XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// As fontes do Word que têm as mesmas medidas das que o FeatherDocs desenha (Liberation).
pub const FONTE_SEM_SERIFA: &str = "Arial";
pub const FONTE_SERIFA: &str = "Times New Roman";
pub const FONTE_CODIGO: &str = "Courier New";

/// O `numId` da numeração dos títulos (as listas usam 1, 2, 3…).
const NUM_TITULOS: u32 = 1000;

/// Os rótulos das legendas (o FeatherDocs numera sozinho; no Word é o campo `SEQ`).
pub const ROTULOS_LEGENDA: [&str; 3] = ["Figura", "Tabela", "Equação"];

/// Os marcadores padrão por nível (o leitor usa a mesma tabela para reconhecer o "Padrão").
pub const MARCADORES_PADRAO: [&str; 3] = ["•", "◦", "▪"];

/// Grava o documento num `.docx`.
pub fn gravar(doc: &Documento) -> Result<Vec<u8>, String> {
    let mut g = Gravador::novo(doc);
    let corpo = g.corpo();
    if !doc.estilos.tema.is_empty() {
        g.rel("theme", "theme/theme1.xml".into(), false);
        g.sacola.push(("word/theme/theme1.xml".into(), "application/vnd.openxmlformats-officedocument.theme+xml".into(), doc.estilos.tema.clone().into_bytes()));
    }
    let mut partes: Vec<(String, Vec<u8>)> = vec![
        ("word/document.xml".into(), corpo.into_bytes()),
        ("word/styles.xml".into(), estilos_xml(doc).into_bytes()),
        ("word/numbering.xml".into(), numeracao_xml(&g.nums, doc.estilos.numeracao).into_bytes()),
        ("word/settings.xml".into(), configuracoes_xml(doc).into_bytes()),
        ("word/header1.xml".into(), faixa_xml("hdr", &doc.extras.cabecalho, &doc.pagina).into_bytes()),
        ("word/footer1.xml".into(), faixa_xml("ftr", &doc.extras.rodape, &doc.pagina).into_bytes()),
        ("docProps/core.xml".into(), propriedades_xml(doc).into_bytes()),
        ("_rels/.rels".into(), raiz_rels().into_bytes()),
    ];
    let notas = [!g.notas[0].is_empty(), !g.notas[1].is_empty()];
    let fontes = !doc.bibliografia.fontes.is_empty();
    let comentarios = !doc.comentarios.is_empty();
    if comentarios {
        let (c, x) = crate::docx_comentarios::gravar(&doc.comentarios);
        partes.push(("word/comments.xml".into(), c.into_bytes()));
        partes.push(("word/commentsExtended.xml".into(), x.into_bytes()));
    }
    if fontes {
        partes.push(("customXml/item1.xml".into(), crate::docx_fontes::gravar(&doc.bibliografia).into_bytes()));
        partes.push(("customXml/itemProps1.xml".into(), crate::docx_fontes::propriedades().into_bytes()));
        partes.push(("customXml/_rels/item1.xml.rels".into(), format!("{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL}/customXmlProps\" Target=\"itemProps1.xml\"/></Relationships>").into_bytes()));
    }
    for (k, (parte, raiz)) in [("word/footnotes.xml", "footnotes"), ("word/endnotes.xml", "endnotes")].into_iter().enumerate() {
        if notas[k] {
            partes.push((parte.into(), notas_xml(raiz, &g.notas[k]).into_bytes()));
        }
    }
    partes.push(("word/_rels/document.xml.rels".into(), g.relacoes_xml(notas, fontes, comentarios).into_bytes()));
    let extensoes: BTreeSet<String> = g.midias.iter().map(|(n, _)| n.rsplit('.').next().unwrap_or("png").to_string()).collect();
    let mut sobrepostos = Vec::new();
    for (caminho, conteudo, bytes) in std::mem::take(&mut g.sacola) {
        if !partes.iter().any(|(n, _)| *n == caminho) {
            if !caminho.ends_with(".rels") {
                let conteudo = if conteudo.is_empty() { "application/octet-stream".to_string() } else { conteudo };
                sobrepostos.push((caminho.clone(), conteudo));
            }
            partes.push((caminho, bytes));
        }
    }
    partes.push(("[Content_Types].xml".into(), tipos_xml(&extensoes, notas, fontes, comentarios, &sobrepostos).into_bytes()));
    for (nome, bytes) in std::mem::take(&mut g.midias) {
        partes.push((format!("word/media/{nome}"), bytes));
    }
    empacotar(partes)
}

fn empacotar(partes: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, String> {
    let erro = |e: &dyn std::fmt::Display| format!("Não consegui gravar o .docx: {e}");
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (nome, bytes) in partes {
        zip.start_file(nome, opcoes).map_err(|e| erro(&e))?;
        zip.write_all(&bytes).map_err(|e| erro(&e))?;
    }
    Ok(zip.finish().map_err(|e| erro(&e))?.into_inner())
}

// ---------------------------------------------------------------------------
// Unidades e escape
// ---------------------------------------------------------------------------

/// Pontos para vigésimos de ponto (a unidade de medida do Word).
fn twips(pt: f32) -> i64 {
    (pt * 20.0).round() as i64
}

/// Pontos para EMU (as imagens).
fn emu(pt: f32) -> i64 {
    (pt * 12700.0).round() as i64
}

/// `0xRRGGBBAA` para `RRGGBB`.
fn hex(cor: u32) -> String {
    format!("{:06X}", cor >> 8)
}

fn esc(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '>' => saida.push_str("&gt;"),
            '"' => saida.push_str("&quot;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => saida.push(c),
        }
    }
    saida
}

/// Uma corrida de texto puro (o `xml:space` guarda os espaços das pontas).
fn t(texto: &str) -> String {
    let partes: Vec<String> = texto.split(featherdocs_core::modelo::QUEBRA_DE_LINHA).map(|p| format!("<w:t xml:space=\"preserve\">{}</w:t>", esc(p))).collect();
    partes.join("<w:br/>")
}

// ---------------------------------------------------------------------------
// O corpo
// ---------------------------------------------------------------------------

/// O tipo de lista, que vira um `abstractNum` (índice na tabela `TIPOS_DE_LISTA`).
fn tipo_de_lista(ordenado: bool, a: &Atrib) -> usize {
    match a.lista {
        Lista::Padrao if ordenado => 1,
        Lista::Padrao => 0,
        Lista::Numerica => 1,
        Lista::Alfabetica => 2,
        Lista::Romana => 3,
        Lista::RomanaMinuscula => 9,
        Lista::Circulo => 4,
        Lista::Quadrado => 5,
        Lista::Seta => 6,
        Lista::Tarefa if a.marcado => 8,
        Lista::Tarefa => 7,
    }
}

/// `(numFmt, lvlText do nível 0 — "%1" é o número; nos marcadores, o caractere)`.
const TIPOS_DE_LISTA: [(&str, &str); 10] = [("bullet", ""), ("decimal", "%1."), ("lowerLetter", "%1)"), ("upperRoman", "%1."), ("bullet", "◦"), ("bullet", "▪"), ("bullet", "➢"), ("bullet", "☐"), ("bullet", "☒"), ("lowerRoman", "%1.")];

/// Uma numeração concreta (`w:num`): o tipo e se recomeça do 1.
struct Num {
    tipo: usize,
    recomeca: bool,
}

struct Gravador<'a> {
    doc: &'a Documento,
    /// `(Id, tipo, alvo, externo)`; os fixos (estilos, numeração…) vêm à parte.
    rels: Vec<(String, String, String, bool)>,
    /// As partes do pacote que os objetos da sacola levam: `(caminho, ContentType, bytes)`.
    sacola: Vec<(String, String, Vec<u8>)>,
    /// Os controles de conteúdo de bloco abertos (o `w:sdt` que embrulha parágrafos inteiros).
    controles: usize,
    midias: Vec<(String, Vec<u8>)>,
    nums: Vec<Num>,
    /// O `numId` em uso por tipo de lista, dentro da lista atual.
    em_uso: [Option<u32>; TIPOS_DE_LISTA.len()],
    anterior_item: bool,
    figuras: u32,
    legendas: [u32; 3],
    /// O XML de cada nota de rodapé (0) e de fim (1), na ordem: o `w:id` é a posição + 1.
    notas: [Vec<String>; 2],
    /// Quantas sugestões já foram (o `w:id` de cada `w:ins`/`w:del`).
    revisoes: u32,
}

impl<'a> Gravador<'a> {
    fn novo(doc: &'a Documento) -> Self {
        Self { doc, rels: Vec::new(), sacola: Vec::new(), controles: 0, midias: Vec::new(), nums: Vec::new(), em_uso: [None; TIPOS_DE_LISTA.len()], anterior_item: false, figuras: 0, legendas: [0; 3], notas: [Vec::new(), Vec::new()], revisoes: 0 }
    }

    /// Uma relação do documento; `tipo` é o fim do `Type` padrão (`image`) ou o `Type` inteiro.
    fn rel(&mut self, tipo: &str, alvo: String, externo: bool) -> String {
        let id = format!("rIdF{}", self.rels.len() + 1);
        let tipo = if tipo.contains("://") { tipo.to_string() } else { format!("{REL}/{tipo}") };
        self.rels.push((id.clone(), tipo, alvo, externo));
        id
    }

    fn corpo(&mut self) -> String {
        let mut s = format!("{XML}<w:document {NS}>");
        if self.doc.extras.cor_pagina != 0 {
            let _ = write!(s, "<w:background w:color=\"{}\"/>", hex(self.doc.extras.cor_pagina));
        }
        s.push_str("<w:body>");
        for b in &self.doc.blocos {
            let x = self.bloco_com_controles(b);
            s.push_str(&x);
        }
        s.push_str(&FIM_DO_CONTROLE.repeat(std::mem::take(&mut self.controles)));
        s.push_str(&secao(self.doc));
        s.push_str("</w:body></w:document>");
        s
    }

    /// O bloco dentro dos controles de conteúdo de bloco que começam (as marcas no começo do texto)
    /// ou terminam (no fim) nele; o controle que ficou no meio do texto sai como controle da linha.
    fn bloco_com_controles(&mut self, b: &Bloco) -> String {
        let de_bloco = |t: &Trecho, fim: bool| matches!(t.marca.as_deref(), Some(Marca::Controle { bloco: true, fim: f, .. }) if *f == fim);
        let texto = match &b.conteudo {
            Conteudo::Titulo { texto, .. } | Conteudo::Paragrafo { texto } | Conteudo::Item { texto, .. } | Conteudo::Citacao { texto } => texto,
            _ => return self.bloco(b),
        };
        let abre = texto.iter().take_while(|t| de_bloco(t, false)).count();
        let fecha = texto[abre..].iter().rev().take_while(|t| de_bloco(t, true)).count();
        if abre + fecha == 0 {
            return self.bloco(b);
        }
        let mut s: String = texto[..abre].iter().map(|t| if let Some(Marca::Controle { xml, .. }) = t.marca.as_deref() { format!("<w:sdt>{xml}<w:sdtContent>") } else { String::new() }).collect();
        self.controles += abre;
        let mut sem = b.clone();
        if let Some(t) = sem.conteudo.textos_mut().pop() {
            t.truncate(t.len() - fecha);
            t.drain(..abre);
        }
        s.push_str(&self.bloco(&sem));
        let n = fecha.min(self.controles);
        self.controles -= n;
        s.push_str(&FIM_DO_CONTROLE.repeat(n));
        s
    }

    fn bloco(&mut self, b: &Bloco) -> String {
        let item = matches!(b.conteudo, Conteudo::Item { .. });
        if !item || !self.anterior_item {
            self.em_uso = [None; TIPOS_DE_LISTA.len()];
        }
        self.anterior_item = item;
        match &b.conteudo {
            Conteudo::Titulo { nivel, texto } => self.titulo(b, *nivel, texto),
            Conteudo::Paragrafo { texto } => self.paragrafo(b, texto),
            Conteudo::Item { ordenado, nivel, texto } => self.item(b, *ordenado, *nivel, texto),
            Conteudo::Citacao { texto } => format!("<w:p>{}{}</w:p>", ppr(Some(&self.estilo_do_word(b).unwrap_or_else(|| "Quote".into())), &b.atrib, ""), self.trechos(texto)),
            Conteudo::Codigo { codigo, .. } => codigo_xml(codigo, &b.atrib),
            Conteudo::Tabela { cabecalho, linhas } => self.tabela(*cabecalho, linhas, &b.mesclagens),
            Conteudo::Imagem { origem, legenda, largura, altura } => self.imagem(origem, legenda, *largura, *altura),
            Conteudo::Equacao { latex } => format!("<w:p>{}<m:oMathPara><m:oMath>{}</m:oMath></m:oMathPara></w:p>", ppr(None, &b.atrib, ""), formula_omml(latex, true)),
            Conteudo::Sumario { niveis, sem_numeros, lista, .. } => sumario_xml(*niveis, *sem_numeros, *lista),
            Conteudo::Regua => "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"auto\"/></w:pBdr></w:pPr></w:p>".into(),
            Conteudo::QuebraDePagina => "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>".into(),
            Conteudo::QuebraDeColuna => "<w:p><w:r><w:br w:type=\"column\"/></w:r></w:p>".into(),
            Conteudo::QuebraDeSecao { inicio } => {
                let tipo = match inicio {
                    InicioDeSecao::ProximaPagina => "nextPage",
                    InicioDeSecao::Continua => "continuous",
                    InicioDeSecao::PaginaPar => "evenPage",
                    InicioDeSecao::PaginaImpar => "oddPage",
                };
                // O w:type vem antes do w:pgSz na ordem do esquema; o papel da seção é o do documento.
                let sec = secao(self.doc).replacen("<w:pgSz", &format!("<w:type w:val=\"{tipo}\"/><w:pgSz"), 1);
                format!("<w:p><w:pPr>{sec}</w:pPr></w:p>")
            }
            Conteudo::Bibliografia { todas } => self.bibliografia(*todas),
            Conteudo::IndiceRemissivo => lista_em_campo(" INDEX \\e \"\t\" \\h \"A\" \\c \"1\" ", "Índice", &[(String::new(), "Atualize o campo (F9) para montar o índice.".to_string())]),
        }
    }

    /// A lista de referências: o campo `BIBLIOGRAPHY` do Word com as referências já no estilo do
    /// documento (o Word, se atualizado, refaz no estilo dele).
    fn bibliografia(&mut self, todas: bool) -> String {
        let b = &self.doc.bibliografia;
        let citadas = featherdocs_core::citacoes::chaves_citadas(&self.doc.blocos);
        let entradas = featherdocs_core::bibliografia::lista(b.estilo, &b.fontes, &citadas, todas, b.sem_sl_sn);
        let recuo = if matches!(b.estilo, EstiloDeCitacao::Abnt) { 0.0 } else { 30.0 };
        let atrib = Atrib { recuo_esq: recuo, primeira: -recuo, depois: Some(8.0), ..Atrib::default() };
        let paragrafos: Vec<(String, String)> = entradas.iter().map(|t| (ppr(None, &atrib, ""), t.iter().map(|t| corrida(t, &self.doc.estilos)).collect())).collect();
        let instr = if todas { " BIBLIOGRAPHY \\l 1046 \\f 1 " } else { " BIBLIOGRAPHY \\l 1046 " };
        lista_em_campo_xml(instr, "Referências", &paragrafos)
    }

    /// O estilo com nome do bloco, se tiver (o `styleId` que `estilos_xml` cria).
    fn estilo_com_nome(&self, b: &Bloco) -> Option<String> {
        self.doc.estilos.do_bloco(b).map(codigo_do_estilo)
    }

    /// O estilo do Word (de parágrafo) do bloco, qualquer que seja o tipo dele (item, citação).
    fn estilo_do_word(&self, b: &Bloco) -> Option<String> {
        self.doc.estilos.pegar(b.atrib.estilo).filter(|e| !e.caractere && !e.codigo.is_empty()).map(codigo_do_estilo)
    }

    /// O `w:pPr` do bloco com estilo do Word (ou do Normal dele): o recuo zero e o alinhamento à
    /// esquerda feitos à mão saem explícitos quando o estilo diz outro (senão, ao ler, o do estilo voltaria).
    fn ppr_do_bloco(&self, estilo: Option<&str>, b: &Bloco, numeracao: &str) -> String {
        let a = &b.atrib;
        let Some(e) = self.doc.estilos.resolvido_do_bloco(b).filter(|e| !e.codigo.is_empty() || a.estilo == 0) else { return ppr(estilo, a, numeracao) };
        let mut forcados = Vec::new();
        if [a.recuo_esq, a.recuo_dir, a.primeira] == [0.0; 3] && [e.recuo_esq, e.recuo_dir, e.primeira] != [0.0; 3] {
            forcados.push("w:ind");
        }
        if a.alinhamento == Alinhamento::Esquerda && e.alinhamento != Alinhamento::Esquerda {
            forcados.push("w:jc");
        }
        // O que o estilo liga (manter com o próximo, sem controle de viúvas…) e o parágrafo não: desligado à vista.
        let desl = Atrib { mais: MaisParagrafo { fluxo: e.mais_paragrafo.fluxo & !a.mais.fluxo, ..MaisParagrafo::default() }, ..Atrib::default() };
        ppr_tudo(estilo, a, numeracao, desl, &forcados)
    }

    fn titulo(&mut self, b: &Bloco, nivel: u8, texto: &[Trecho]) -> String {
        let estilo = self.estilo_com_nome(b).unwrap_or_else(|| format!("Heading{}", nivel.clamp(1, 6)));
        let extra = if b.atrib.sem_numero && self.doc.estilos.numeracao != Numeracao::Nenhuma { "<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"0\"/></w:numPr>" } else { "" };
        format!("<w:p>{}{}</w:p>", self.ppr_do_bloco(Some(&estilo), b, extra), self.trechos(texto))
    }

    fn paragrafo(&mut self, b: &Bloco, texto: &[Trecho]) -> String {
        if b.atrib.legenda > 0 {
            return self.legenda(b, texto);
        }
        let estilo = self.estilo_com_nome(b);
        format!("<w:p>{}{}</w:p>", self.ppr_do_bloco(estilo.as_deref(), b, ""), self.trechos(texto))
    }

    /// "Figura 1 – texto": o número é o campo `SEQ`, que o Word recalcula.
    fn legenda(&mut self, b: &Bloco, texto: &[Trecho]) -> String {
        let i = usize::from(b.atrib.legenda.clamp(1, 3)) - 1;
        self.legendas[i] += 1;
        let rotulo = ROTULOS_LEGENDA[i];
        let campo = format!("<w:fldSimple w:instr=\" SEQ {rotulo} \\* ARABIC \"><w:r>{}</w:r></w:fldSimple>", t(&self.legendas[i].to_string()));
        format!("<w:p>{}<w:r>{}</w:r>{campo}<w:r>{}</w:r>{}</w:p>", ppr(Some("Caption"), &b.atrib, ""), t(&format!("{rotulo} ")), t(" – "), self.trechos(texto))
    }

    fn item(&mut self, b: &Bloco, ordenado: bool, nivel: u8, texto: &[Trecho]) -> String {
        let tipo = tipo_de_lista(ordenado, &b.atrib);
        if b.atrib.reiniciar {
            self.em_uso[tipo] = None;
        }
        let num = match self.em_uso[tipo] {
            Some(n) => n,
            None => {
                let recomeca = self.nums.iter().any(|n| n.tipo == tipo);
                self.nums.push(Num { tipo, recomeca });
                let n = self.nums.len() as u32;
                self.em_uso[tipo] = Some(n);
                n
            }
        };
        let extra = format!("<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{num}\"/></w:numPr>", nivel.min(8));
        // O recuo do item soma ao do nível (`nivel_de_lista`): o `w:ind` do parágrafo troca o da numeração.
        let mut atrib = b.atrib;
        if atrib.recuo_esq != 0.0 {
            atrib.recuo_esq += RECUO_DO_NIVEL * f32::from(nivel.min(8) + 1);
        }
        // O item com o estilo que veio do Word volta com ele; sem estilo, o do FeatherDocs (se o do Word não veio).
        let estilo = self.estilo_do_word(b).or_else(|| (!self.doc.estilos.lista.iter().any(|e| e.codigo == "ListParagraph")).then(|| "ListParagraph".to_string()));
        format!("<w:p>{}{}</w:p>", ppr(estilo.as_deref(), &atrib, &extra), self.trechos(texto))
    }

    fn trechos(&mut self, ts: &[Trecho]) -> String {
        let mut s = String::new();
        // Os controles de conteúdo abertos na linha: o que não fecha nela fecha no fim.
        let mut controles = 0;
        for tr in ts {
            if let Some(Marca::Controle { xml, fim, .. }) = tr.marca.as_deref() {
                if !*fim {
                    controles += 1;
                    let _ = write!(s, "<w:sdt>{xml}<w:sdtContent>");
                } else if controles > 0 {
                    controles -= 1;
                    s.push_str(FIM_DO_CONTROLE);
                }
                continue;
            }
            if let Some(m) = tr.marca.as_deref() {
                let x = self.objeto(tr, m);
                s.push_str(&x);
                continue;
            }
            let corrida = self.com_revisao(tr, corrida(tr, &self.doc.estilos));
            match &tr.link {
                Some(u) if u.starts_with('#') => {
                    let _ = write!(s, "<w:hyperlink w:anchor=\"{}\">{corrida}</w:hyperlink>", esc(&u[1..]));
                }
                Some(u) => {
                    let id = self.rel("hyperlink", u.clone(), true);
                    let _ = write!(s, "<w:hyperlink r:id=\"{id}\">{corrida}</w:hyperlink>");
                }
                None => s.push_str(&corrida),
            }
        }
        s.push_str(&FIM_DO_CONTROLE.repeat(controles));
        s
    }

    /// A corrida dentro de `w:ins` ou `w:del` quando o trecho é uma sugestão (o texto apagado vai em `w:delText`).
    fn com_revisao(&mut self, tr: &Trecho, corrida: String) -> String {
        let Some(r) = tr.rev.as_deref() else { return corrida };
        self.revisoes += 1;
        let data = crate::docx_comentarios::data_iso(&r.quando);
        let data = if data.is_empty() { String::new() } else { format!(" w:date=\"{data}\"") };
        let autor = if r.autor.is_empty() { "FeatherDocs" } else { &r.autor };
        match r.tipo {
            TipoDeRevisao::Insercao => format!("<w:ins w:id=\"{}\" w:author=\"{}\"{data}>{corrida}</w:ins>", self.revisoes, esc(autor)),
            TipoDeRevisao::Remocao => format!("<w:del w:id=\"{}\" w:author=\"{}\"{data}>{}</w:del>", self.revisoes, esc(autor), corrida.replace("<w:t ", "<w:delText ").replace("<w:t>", "<w:delText>").replace("</w:t>", "</w:delText>")),
        }
    }

    /// Um objeto preso ao texto: a chamada da nota (e o texto dela vai para `footnotes.xml`), o campo
    /// `XE` do índice, o campo de dados ou a citação.
    fn objeto(&mut self, tr: &Trecho, m: &Marca) -> String {
        match m {
            Marca::Nota { fim, texto } => {
                let k = usize::from(*fim);
                let id = self.notas[k].len() + 1;
                let (tag, marca) = if *fim { ("endnote", "endnoteRef") } else { ("footnote", "footnoteRef") };
                let corpo: String = texto.iter().filter(|t| t.marca.is_none()).map(|t| corrida(t, &self.doc.estilos)).collect();
                let sobrescrito = "<w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr>";
                self.notas[k].push(format!("<w:{tag} w:id=\"{id}\"><w:p><w:r>{sobrescrito}<w:{marca}/></w:r><w:r><w:t xml:space=\"preserve\"> </w:t></w:r>{corpo}</w:p></w:{tag}>"));
                // A chamada leva a letra do trecho (o estilo de caractere da chamada, vindo do Word), sempre sobrescrita.
                let chamada = Trecho { estilo: tr.estilo, fmt: Fmt { posicao: 1, ..tr.fmt }, ..Trecho::default() };
                format!("<w:r>{}<w:{tag}Reference w:id=\"{id}\"/></w:r>", rpr(&chamada, &self.doc.estilos))
            }
            Marca::Indice { termo, sub } => {
                let entrada = if sub.is_empty() { termo.clone() } else { format!("{termo}:{sub}") };
                campo_complexo(&format!(" XE \"{}\" ", entrada.replace('"', "'")), "")
            }
            Marca::Campo { campo } => {
                let instr = match campo.as_str() {
                    "autor" => "AUTHOR",
                    "titulo" => "TITLE",
                    "data" => "DATE \\@ \"dd/MM/yyyy\"",
                    "pagina" => "PAGE",
                    "paginas" => "NUMPAGES",
                    _ => "FILENAME",
                };
                format!("<w:fldSimple w:instr=\" {} \"><w:r>{}{}</w:r></w:fldSimple>", esc(instr), rpr(tr, &self.doc.estilos), t(&tr.rotulo))
            }
            Marca::Comentario { id, fim } => {
                let ids = self.doc.comentarios.iter().find(|c| c.id == *id).map(crate::docx_comentarios::ids).unwrap_or_else(|| vec![*id]);
                ids.iter()
                    .map(|i| if *fim { format!("<w:commentRangeEnd w:id=\"{i}\"/><w:r><w:commentReference w:id=\"{i}\"/></w:r>") } else { format!("<w:commentRangeStart w:id=\"{i}\"/>") })
                    .collect()
            }
            Marca::Citacao { chaves, pagina } => {
                let p = if pagina.is_empty() { String::new() } else { format!(" \\p {pagina}") };
                let rotulo = if tr.rotulo.is_empty() { format!("({})", chaves.join("; ")) } else { tr.rotulo.clone() };
                format!("<w:fldSimple w:instr=\" CITATION {}{} \"><w:r>{}{}</w:r></w:fldSimple>", esc(&chaves.join(" \\m ")), esc(&p), rpr(tr, &self.doc.estilos), t(&rotulo))
            }
            // shortcut: a imagem no meio da linha não vai para o DOCX; gravar como `w:drawing` em linha quando pedirem.
            Marca::Imagem { .. } | Marca::Controle { .. } => String::new(),
            Marca::ObjetoWord { xml, partes, .. } => format!("<w:r>{}{}</w:r>", rpr(tr, &self.doc.estilos), self.sacola(xml, partes)),
        }
    }

    /// O XML de um objeto da sacola, com os `r:id` trocados pelas relações novas, e as partes dele no pacote.
    fn sacola(&mut self, xml: &str, partes: &[featherdocs_core::modelo::ParteWord]) -> String {
        let mut novos = std::collections::HashMap::new();
        for p in partes {
            if !p.rid.is_empty() {
                let alvo = if p.externo { p.caminho.clone() } else { p.caminho.strip_prefix("word/").map_or_else(|| format!("/{}", p.caminho), str::to_string) };
                novos.insert(p.rid.clone(), self.rel(&p.tipo, alvo, p.externo));
            }
            // shortcut: duas partes com o mesmo caminho (objeto colado de outro documento) gravam a
            // primeira; renomear a parte quando os bytes diferirem.
            if !p.externo && !p.bytes.is_empty() && !self.sacola.iter().any(|(c, _, _)| *c == p.caminho) {
                self.sacola.push((p.caminho.clone(), p.conteudo.clone(), p.bytes.clone()));
            }
        }
        crate::docx::sacola::trocar_rids(xml, &novos)
    }

    fn tabela(&mut self, cabecalho: bool, linhas: &[Vec<Celula>], mesclagens: &[featherdocs_core::modelo::MesclagemTabela]) -> String {
        let cols = linhas.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let largura = twips(self.doc.pagina.largura_util()) / cols as i64;
        let borda = |lado: &str| format!("<w:{lado} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"BFBFBF\"/>");
        let bordas: String = ["top", "left", "bottom", "right", "insideH", "insideV"].iter().map(|l| borda(l)).collect();
        let mut s = format!("<w:tbl><w:tblPr><w:tblW w:w=\"{}\" w:type=\"dxa\"/><w:tblBorders>{bordas}</w:tblBorders><w:tblCellMar><w:left w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"80\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>", largura * cols as i64);
        for _ in 0..cols {
            let _ = write!(s, "<w:gridCol w:w=\"{largura}\"/>");
        }
        s.push_str("</w:tblGrid>");
        for (i, linha) in linhas.iter().enumerate() {
            let topo = cabecalho && i == 0;
            s.push_str(if topo { "<w:tr><w:trPr><w:tblHeader/></w:trPr>" } else { "<w:tr>" });
            let mut c = 0;
            while c < cols {
                let regiao = mesclagens.iter().find(|m| m.linha == i && m.coluna == c
                    && m.colunas >= 2 && c.checked_add(m.colunas).is_some_and(|fim| fim <= cols));
                let n = regiao.map_or(1, |m| m.colunas);
                let fundo = if topo { "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F2F2F2\"/>" } else { "" };
                let span = if n > 1 { format!("<w:gridSpan w:val=\"{n}\"/>") } else { String::new() };
                let texto = linha.get(c).map(|cel| self.trechos(cel)).unwrap_or_default();
                let _ = write!(s, "<w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/>{fundo}{span}</w:tcPr><w:p><w:pPr><w:spacing w:before=\"40\" w:after=\"40\"/></w:pPr>{texto}</w:p></w:tc>", largura * n as i64);
                c += n;
            }
            s.push_str("</w:tr>");
        }
        s.push_str("</w:tbl>");
        s
    }

    fn imagem(&mut self, origem: &str, legenda: &[Trecho], largura: f32, altura: f32) -> String {
        let (w, h) = if largura > 0.0 && altura > 0.0 { (largura, altura) } else { (300.0, 200.0) };
        let util = self.doc.pagina.largura_util();
        let (w, h) = if w > util { (util, h * util / w) } else { (w, h) };
        let blip = if let Some((tipo, bytes)) = base64::de_data_url(origem) {
            self.figuras += 1;
            let ext = match tipo.as_str() {
                "image/jpeg" | "image/jpg" => "jpeg",
                "image/gif" => "gif",
                "image/bmp" => "bmp",
                "image/svg+xml" => "svg",
                "image/webp" => "webp",
                _ => "png",
            };
            let nome = format!("imagem{}.{ext}", self.figuras);
            self.midias.push((nome.clone(), bytes));
            format!("r:embed=\"{}\"", self.rel("image", format!("media/{nome}"), false))
        } else if origem.starts_with("http://") || origem.starts_with("https://") {
            self.figuras += 1;
            format!("r:link=\"{}\"", self.rel("image", origem.to_string(), true))
        } else {
            return format!("<w:p><w:r>{}</w:r></w:p>", t(&format!("[imagem: {origem}]")));
        };
        let n = self.figuras;
        let (cx, cy) = (emu(w), emu(h));
        let desenho = format!(
            "<w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{n}\" name=\"Imagem {n}\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"Imagem {n}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip {blip}/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"
        );
        let mut s = format!("<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r>{desenho}</w:r></w:p>");
        if !legenda.is_empty() {
            let _ = write!(s, "<w:p><w:pPr><w:pStyle w:val=\"FDLegendaImagem\"/></w:pPr>{}</w:p>", self.trechos(legenda));
        }
        s
    }

    fn relacoes_xml(&self, notas: [bool; 2], fontes: bool, comentarios: bool) -> String {
        let mut s = format!("{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
        for (id, tipo, alvo) in [("rIdEstilos", "styles", "styles.xml"), ("rIdNumeracao", "numbering", "numbering.xml"), ("rIdConfig", "settings", "settings.xml"), ("rIdCabecalho", "header", "header1.xml"), ("rIdRodape", "footer", "footer1.xml")] {
            let _ = write!(s, "<Relationship Id=\"{id}\" Type=\"{REL}/{tipo}\" Target=\"{alvo}\"/>");
        }
        for (k, (id, tipo, alvo)) in [("rIdNotas", "footnotes", "footnotes.xml"), ("rIdNotasFim", "endnotes", "endnotes.xml")].into_iter().enumerate() {
            if notas[k] {
                let _ = write!(s, "<Relationship Id=\"{id}\" Type=\"{REL}/{tipo}\" Target=\"{alvo}\"/>");
            }
        }
        if fontes {
            let _ = write!(s, "<Relationship Id=\"rIdFontes\" Type=\"{REL}/customXml\" Target=\"../customXml/item1.xml\"/>");
        }
        if comentarios {
            let _ = write!(s, "<Relationship Id=\"rIdComentarios\" Type=\"{REL}/comments\" Target=\"comments.xml\"/>");
            s.push_str("<Relationship Id=\"rIdComentariosEx\" Type=\"http://schemas.microsoft.com/office/2011/relationships/commentsExtended\" Target=\"commentsExtended.xml\"/>");
        }
        for (id, tipo, alvo, externo) in &self.rels {
            let modo = if *externo { " TargetMode=\"External\"" } else { "" };
            let _ = write!(s, "<Relationship Id=\"{id}\" Type=\"{}\" Target=\"{}\"{modo}/>", esc(tipo), esc(alvo));
        }
        s.push_str("</Relationships>");
        s
    }
}

const FIM_DO_CONTROLE: &str = "</w:sdtContent></w:sdt>";

/// As propriedades de parágrafo, na ordem que o esquema do Word exige.
fn ppr(estilo: Option<&str>, a: &Atrib, numeracao: &str) -> String {
    ppr_tudo(estilo, a, numeracao, Atrib::default(), &[])
}

/// `ppr`, com os elementos do mapa em `forcados` gravados mesmo com o valor padrão e o que `desl`
/// desliga à vista.
fn ppr_tudo(estilo: Option<&str>, a: &Atrib, numeracao: &str, desl: Atrib, forcados: &[&str]) -> String {
    let mut s = String::new();
    if let Some(e) = estilo {
        let _ = write!(s, "<w:pStyle w:val=\"{e}\"/>");
    }
    s.push_str(&paragrafo_xml(a, desl, forcados, numeracao));
    if s.is_empty() {
        s
    } else {
        format!("<w:pPr>{s}</w:pPr>")
    }
}

/// O miolo do `w:pPr` (sem o `w:pStyle`): o mapa, com a numeração, as bordas e as tabulações no lugar
/// delas; `desl` = o que o estilo desliga à vista.
fn paragrafo_xml(a: &Atrib, desl: Atrib, forcados: &[&str], numeracao: &str) -> String {
    let (bordas, tabs) = (bordas_do_paragrafo(a), tabulacoes_xml(a));
    mapa::gravar_tudo(mapa::PARAGRAFO, *a, desl, forcados, &[("w:numPr", numeracao), ("w:pBdr", &bordas), ("w:tabs", &tabs)])
}

fn bordas_do_paragrafo(a: &Atrib) -> String {
    if a.borda == 0 {
        return String::new();
    }
    let cor = if a.cor_borda != 0 { hex(a.cor_borda) } else { "auto".into() };
    let traco = mapa::TRACOS_DE_BORDA.get(usize::from(a.mais.borda_tipo)).unwrap_or(&"single");
    let esp = if a.mais.borda_espessura > 0.0 { a.mais.borda_espessura } else { crate::docx::ESPESSURA_DA_BORDA };
    let sz = (esp * 8.0).round() as i64;
    let mut s = String::from("<w:pBdr>");
    for (bit, lado) in [(4, "top"), (1, "left"), (8, "bottom"), (2, "right")] {
        if a.borda & bit != 0 {
            let _ = write!(s, "<w:{lado} w:val=\"{traco}\" w:sz=\"{sz}\" w:space=\"4\" w:color=\"{cor}\"/>");
        }
    }
    s.push_str("</w:pBdr>");
    s
}

/// As paradas de tabulação (`w:tabs`); vazio sem nenhuma.
fn tabulacoes_xml(a: &Atrib) -> String {
    let paradas = a.mais.paradas();
    if paradas.is_empty() {
        return String::new();
    }
    let mut s = String::from("<w:tabs>");
    for t in paradas {
        let _ = write!(s, "<w:tab w:val=\"{}\"", mapa::ALINHAMENTOS_DE_TABULACAO.get(usize::from(t.alinhamento)).unwrap_or(&"left"));
        if let Some(p) = mapa::PREENCHIMENTOS_DE_TABULACAO.get(usize::from(t.preenchimento)).filter(|_| t.preenchimento != 0) {
            let _ = write!(s, " w:leader=\"{p}\"");
        }
        let _ = write!(s, " w:pos=\"{}\"/>", twips(t.posicao));
    }
    s.push_str("</w:tabs>");
    s
}

/// As propriedades de caractere de um trecho (vazio = nenhuma).
fn rpr(tr: &Trecho, estilos: &Estilos) -> String {
    let estilo = estilos.pegar(tr.fmt.estilo_de_caractere).filter(|e| e.caractere).map(codigo_do_estilo).or_else(|| tr.link.is_some().then(|| "Hyperlink".to_string()));
    let s = caractere_xml(estilo.as_deref(), tr.estilo, tr.fmt, 0);
    if s.is_empty() {
        s
    } else {
        format!("<w:rPr>{s}</w:rPr>")
    }
}

/// O miolo do `w:rPr`: o estilo de caractere, a fonte (pelo nome, ou pela classe sem nome) e o mapa;
/// `desligados` = o que o estilo desliga à vista (`Estilo::desligados`).
fn caractere_xml(estilo: Option<&str>, e: u8, fmt: Fmt, desligados: u32) -> String {
    let mut s = String::new();
    if let Some(r) = estilo {
        let _ = write!(s, "<w:rStyle w:val=\"{}\"/>", esc(r));
    }
    let fonte = if e & CODIGO != 0 { Some(FONTE_CODIGO) } else if e & SERIFA != 0 { Some(FONTE_SERIFA) } else { None };
    if let (Some(f), true) = (fonte, fmt.familia.vazia()) {
        let _ = write!(s, "<w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\" w:cs=\"{f}\"/>");
    }
    let [bits, efeitos, ..] = desligados.to_le_bytes();
    let desl = (bits, Fmt { mais: MaisLetra { efeitos, ..MaisLetra::default() }, ..Fmt::default() });
    s.push_str(&mapa::gravar_tudo(mapa::CARACTERE, (e, fmt), desl, &[], &[]));
    s
}

/// O `w:pPr` de um estilo (sem o elemento em volta), com o que ele desliga à vista.
fn paragrafo_do_estilo_xml(e: &Estilo, a: &Atrib, num: &str) -> String {
    let desl = Atrib { mais: MaisParagrafo { fluxo: e.desligados.to_le_bytes()[2], ..MaisParagrafo::default() }, ..Atrib::default() };
    let forcados: &[&str] = if e.desligados & ALINHAMENTO_DEFINIDO != 0 { &["w:jc"] } else { &[] };
    paragrafo_xml(a, desl, forcados, num)
}

/// O `styleId` de um estilo da lista: o do Word, ou `FDEstilo<id>` (para o leitor achar o id de volta).
fn codigo_do_estilo(e: &Estilo) -> String {
    if e.codigo.is_empty() {
        format!("FDEstilo{}", e.id)
    } else {
        e.codigo.clone()
    }
}

/// A fórmula como equação nativa do Word (OMML, do `feather-math`); vazia, o LaTeX cru numa corrida.
fn formula_omml(latex: &str, exibida: bool) -> String {
    let x = feather_math::omml::omml(latex, exibida);
    if x.is_empty() {
        format!("<m:r><m:t>{}</m:t></m:r>", esc(latex))
    } else {
        x
    }
}

/// Uma corrida (`w:r`): o texto com tabulações e quebras de linha; fórmula em linha vira `m:oMath`.
fn corrida(tr: &Trecho, estilos: &Estilos) -> String {
    if tr.estilo & MATEMATICA != 0 {
        return format!("<m:oMath>{}</m:oMath>", formula_omml(&tr.texto, false));
    }
    let mut s = format!("<w:r>{}", rpr(tr, estilos));
    for (i, linha) in tr.texto.split('\n').enumerate() {
        if i > 0 {
            s.push_str("<w:br/>");
        }
        for (j, pedaco) in linha.split('\t').enumerate() {
            if j > 0 {
                s.push_str("<w:tab/>");
            }
            if !pedaco.is_empty() {
                s.push_str(&t(pedaco));
            }
        }
    }
    s.push_str("</w:r>");
    s
}

/// O bloco de código: um parágrafo só, com as linhas separadas por `w:br`.
fn codigo_xml(codigo: &str, a: &Atrib) -> String {
    let mut s = format!("<w:p>{}<w:r>", ppr(Some("Code"), a, ""));
    for (i, linha) in codigo.split('\n').enumerate() {
        if i > 0 {
            s.push_str("<w:br/>");
        }
        s.push_str(&t(linha));
    }
    s.push_str("</w:r></w:p>");
    s
}

/// O sumário (ou a lista de figuras/tabelas) como campo `TOC`; o Word o preenche ao abrir.
/// Uma lista gerada (bibliografia, índice) como um campo complexo que ocupa vários parágrafos: o
/// título e os parágrafos são o resultado do campo (o leitor joga fora e refaz).
fn lista_em_campo(instr: &str, titulo: &str, linhas: &[(String, String)]) -> String {
    let paragrafos: Vec<(String, String)> = linhas.iter().map(|(ppr, texto)| (ppr.clone(), format!("<w:r>{}</w:r>", t(texto)))).collect();
    lista_em_campo_xml(instr, titulo, &paragrafos)
}

/// Como `lista_em_campo`, com as corridas já em XML: `(pPr, corridas)` por parágrafo.
fn lista_em_campo_xml(instr: &str, titulo: &str, paragrafos: &[(String, String)]) -> String {
    let mut s = format!(
        "<w:p><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\">{}</w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:rPr><w:b/><w:sz w:val=\"32\"/></w:rPr>{}</w:r></w:p>",
        esc(instr),
        t(titulo)
    );
    for (ppr, corridas) in paragrafos {
        let _ = write!(s, "<w:p>{ppr}{corridas}</w:p>");
    }
    s.push_str("<w:p><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>");
    s
}

fn sumario_xml(niveis: u8, sem_numeros: bool, lista: u8) -> String {
    let (titulo, instr) = match lista {
        1 => ("Lista de figuras", format!("TOC \\h \\z \\c \"{}\"", ROTULOS_LEGENDA[0])),
        2 => ("Lista de tabelas", format!("TOC \\h \\z \\c \"{}\"", ROTULOS_LEGENDA[1])),
        _ => ("Sumário", format!("TOC \\o \"1-{}\" \\h \\z \\u", niveis.clamp(1, 9))),
    };
    let instr = if sem_numeros { format!("{instr} \\n") } else { instr };
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"TOCHeading\"/></w:pPr><w:r>{}</w:r></w:p><w:p><w:fldSimple w:instr=\" {} \"><w:r>{}</w:r></w:fldSimple></w:p>",
        t(titulo),
        esc(&instr),
        t("Atualize o campo (F9) para montar o sumário.")
    )
}

/// As propriedades da seção: cabeçalho, rodapé, papel, margens, borda, números de linha, alinhamento vertical.
fn secao(doc: &Documento) -> String {
    let p = &doc.pagina;
    let x = &doc.extras;
    let mut s = String::from("<w:sectPr><w:headerReference w:type=\"default\" r:id=\"rIdCabecalho\"/><w:footerReference w:type=\"default\" r:id=\"rIdRodape\"/>");
    let paisagem = if p.largura > p.altura { " w:orient=\"landscape\"" } else { "" };
    let _ = write!(s, "<w:pgSz w:w=\"{}\" w:h=\"{}\"{paisagem}/>", twips(p.largura), twips(p.altura));
    let _ = write!(s, "<w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/>", twips(p.margem_sup), twips(p.margem_dir), twips(p.margem_inf), twips(p.margem_esq));
    if x.borda_cor != 0 {
        let sz = ((x.borda_espessura.max(0.25) * 8.0).round() as i64).clamp(2, 96);
        let espaco = (x.borda_distancia.round() as i64).clamp(0, 31);
        let lado = |l: &str| format!("<w:{l} w:val=\"single\" w:sz=\"{sz}\" w:space=\"{espaco}\" w:color=\"{}\"/>", hex(x.borda_cor));
        let _ = write!(s, "<w:pgBorders w:offsetFrom=\"page\">{}{}{}{}</w:pgBorders>", lado("top"), lado("left"), lado("bottom"), lado("right"));
    }
    match x.numerar_linhas {
        NumeracaoDeLinhas::Nenhuma => {}
        NumeracaoDeLinhas::Continua => s.push_str("<w:lnNumType w:countBy=\"1\" w:restart=\"continuous\"/>"),
        NumeracaoDeLinhas::PorPagina => s.push_str("<w:lnNumType w:countBy=\"1\" w:restart=\"newPage\"/>"),
    }
    match x.vertical {
        Vertical::Topo => {}
        Vertical::Centro => s.push_str("<w:vAlign w:val=\"center\"/>"),
        Vertical::Base => s.push_str("<w:vAlign w:val=\"bottom\"/>"),
    }
    if x.primeira_diferente {
        s.push_str("<w:titlePg/>");
    }
    s.push_str("</w:sectPr>");
    s
}

// ---------------------------------------------------------------------------
// As partes de apoio
// ---------------------------------------------------------------------------

/// Cabeçalho ou rodapé: esquerda, centro e direita separados por tabulação (centro e direita).
fn faixa_xml(raiz: &str, f: &Faixas, p: &featherdocs_core::modelo::ConfigPagina) -> String {
    let util = twips(p.largura_util());
    let mut s = format!("{XML}<w:{raiz} {NS}>");
    if f.vazia() {
        let _ = write!(s, "<w:p/></w:{raiz}>");
        return s;
    }
    let _ = write!(s, "<w:p><w:pPr><w:tabs><w:tab w:val=\"center\" w:pos=\"{}\"/><w:tab w:val=\"right\" w:pos=\"{util}\"/></w:tabs></w:pPr>", util / 2);
    for (i, texto) in [&f.esquerda, &f.centro, &f.direita].into_iter().enumerate() {
        if i > 0 {
            s.push_str("<w:r><w:tab/></w:r>");
        }
        s.push_str(&texto_com_campos(texto));
    }
    let _ = write!(s, "</w:p></w:{raiz}>");
    s
}

/// `{pagina}`, `{total}`, `{pagina_romano}`, `{titulo}` e `{autor}` viram campos do Word.
pub(crate) const CAMPOS: [(&str, &str); 5] = [("{pagina}", "PAGE"), ("{total}", "NUMPAGES"), ("{pagina_romano}", "PAGE \\* roman"), ("{titulo}", "TITLE"), ("{autor}", "AUTHOR")];

fn texto_com_campos(texto: &str) -> String {
    let mut s = String::new();
    let mut resto = texto;
    while !resto.is_empty() {
        let proximo = CAMPOS.iter().filter_map(|(marca, instr)| resto.find(marca).map(|i| (i, *marca, *instr))).min_by_key(|(i, ..)| *i);
        let Some((i, marca, instr)) = proximo else {
            let _ = write!(s, "<w:r>{}</w:r>", t(resto));
            break;
        };
        if i > 0 {
            let _ = write!(s, "<w:r>{}</w:r>", t(&resto[..i]));
        }
        let _ = write!(s, "<w:fldSimple w:instr=\" {} \"><w:r>{}</w:r></w:fldSimple>", esc(instr), t("1"));
        resto = &resto[i + marca.len()..];
    }
    s
}

fn estilo_de_paragrafo(id: &str, nome: &str, base: Option<&str>, ppr: &str, rpr: &str) -> String {
    let base = base.map(|b| format!("<w:basedOn w:val=\"{b}\"/><w:next w:val=\"Normal\"/>")).unwrap_or_default();
    format!("<w:style w:type=\"paragraph\" w:styleId=\"{id}\"><w:name w:val=\"{}\"/>{base}<w:qFormat/><w:pPr>{ppr}</w:pPr><w:rPr>{rpr}</w:rPr></w:style>", esc(nome))
}

/// Os estilos que o gravador põe por conta própria (o leitor não traz de volta os que vierem iguais).
pub(crate) fn estilos_embutidos() -> String {
    estilos_xml(&Documento::vazio())
}

pub(crate) fn estilos_xml(doc: &Documento) -> String {
    let fonte = |f: &str| format!("<w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\" w:cs=\"{f}\"/>");
    let p = &doc.estilos.padrao;
    let (rpr_padrao, ppr_padrao) = if p.eh_padrao() {
        (format!("{}<w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/>", fonte(FONTE_SEM_SERIFA)), "<w:spacing w:after=\"120\" w:line=\"276\" w:lineRule=\"auto\"/>".to_string())
    } else {
        let (bits, fmt) = p.caractere_do_estilo();
        (caractere_xml(None, bits, fmt, p.desligados), paragrafo_do_estilo_xml(p, &p.paragrafo_do_estilo(), ""))
    };
    let mut s = format!("{XML}<w:styles {NS}><w:docDefaults><w:rPrDefault><w:rPr>{rpr_padrao}<w:lang w:val=\"pt-BR\"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>{ppr_padrao}</w:pPr></w:pPrDefault></w:docDefaults>");
    // O estilo do mesmo `styleId` vindo do Word toma o lugar do embutido.
    let tem = |c: &str| doc.estilos.lista.iter().any(|e| e.codigo == c);
    let mut embutido = |id: &str, xml: String| {
        if !tem(id) {
            s.push_str(&xml);
        }
    };
    embutido("Normal", "<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>".into());
    let numerados = doc.estilos.numeracao != Numeracao::Nenhuma;
    for n in 1..=6u8 {
        let tam = (tamanho_base(&Conteudo::Titulo { nivel: n, texto: Vec::new() }) * 2.0).round() as i64;
        let ppr = format!("<w:keepNext/>{}<w:spacing w:before=\"240\" w:after=\"120\"/><w:outlineLvl w:val=\"{}\"/>", numero_do_titulo(numerados, n), n - 1);
        embutido(&format!("Heading{n}"), estilo_de_paragrafo(&format!("Heading{n}"), &format!("heading {n}"), Some("Normal"), &ppr, &format!("<w:b/><w:sz w:val=\"{tam}\"/><w:szCs w:val=\"{tam}\"/>")));
    }
    embutido("Quote", estilo_de_paragrafo("Quote", "Quote", Some("Normal"), "<w:ind w:left=\"440\"/>", "<w:i/><w:color w:val=\"595959\"/>"));
    embutido("Code", estilo_de_paragrafo("Code", "Code", Some("Normal"), "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F5F5F5\"/><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/>", &format!("{}<w:sz w:val=\"19\"/>", fonte(FONTE_CODIGO))));
    embutido("ListParagraph", estilo_de_paragrafo("ListParagraph", "List Paragraph", Some("Normal"), "<w:spacing w:after=\"40\"/>", ""));
    embutido("Caption", estilo_de_paragrafo("Caption", "caption", Some("Normal"), "", "<w:i/><w:sz w:val=\"18\"/>"));
    embutido("FDLegendaImagem", estilo_de_paragrafo("FDLegendaImagem", "Legenda da imagem", Some("Normal"), "<w:jc w:val=\"center\"/>", "<w:i/><w:sz w:val=\"18\"/>"));
    embutido("TOCHeading", estilo_de_paragrafo("TOCHeading", "TOC Heading", Some("Normal"), "<w:keepNext/><w:spacing w:before=\"240\" w:after=\"120\"/>", "<w:b/><w:sz w:val=\"28\"/>"));
    embutido("Hyperlink", "<w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/><w:rPr><w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/></w:rPr></w:style>".into());
    for e in &doc.estilos.lista {
        s.push_str(&estilo_xml(e, &doc.estilos));
    }
    s.push_str("</w:styles>");
    s
}

/// O `w:numPr` do estilo do título `n` quando os títulos são numerados.
fn numero_do_titulo(numerados: bool, n: u8) -> String {
    if numerados {
        format!("<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{NUM_TITULOS}\"/></w:numPr>", n - 1)
    } else {
        String::new()
    }
}

/// Um estilo da lista: o do usuário ou o vindo do Word, de parágrafo ou de caractere, com a herança
/// (`w:basedOn`), o próximo (`w:next`) e o que ele define da letra e do parágrafo.
fn estilo_xml(e: &Estilo, l: &Estilos) -> String {
    let codigo = |id: u32| l.pegar(id).map(codigo_do_estilo);
    // O título que o Word reconhece pelo nome (o nível vem dele, sem `w:outlineLvl`).
    let titulo_pelo_nome = e.codigo.strip_prefix("Heading").is_some_and(|n| n.parse::<u8>().is_ok()) || e.codigo == "Title" || e.codigo == "Subtitle";
    let base = match codigo(e.herda) {
        Some(c) => Some(c),
        None if e.caractere || e.codigo == "Normal" => None,
        // O do FeatherDocs se apoia no tipo da base (é dele que vem o nível no Word).
        None if e.codigo.is_empty() => Some(if e.base == 0 { "Normal".to_string() } else { format!("Heading{}", e.base.clamp(1, 6)) }),
        // O do Word sem pai na lista: o Normal embutido (o leitor não o traz de volta), se o Normal não veio.
        None if l.lista.iter().any(|x| x.codigo == "Normal") => None,
        None => Some("Normal".to_string()),
    };
    // O nível que não vem do nome nem do pai sai no `w:outlineLvl`.
    let mut a = e.paragrafo_do_estilo();
    if !e.codigo.is_empty() && e.base > 0 && !titulo_pelo_nome && l.pegar(e.herda).map_or(0, |p| p.base) != e.base {
        a.mais.nivel_topico = e.base;
    }
    let mut s = format!("<w:style w:type=\"{}\"", if e.caractere { "character" } else { "paragraph" });
    if e.codigo.is_empty() {
        s.push_str(" w:customStyle=\"1\"");
    }
    let _ = write!(s, " w:styleId=\"{}\"><w:name w:val=\"{}\"/>", esc(&codigo_do_estilo(e)), esc(&e.nome));
    if let Some(b) = base {
        let _ = write!(s, "<w:basedOn w:val=\"{}\"/>", esc(&b));
    }
    if !e.caractere {
        if let Some(p) = codigo(e.proximo).or_else(|| e.codigo.is_empty().then(|| "Normal".to_string())) {
            let _ = write!(s, "<w:next w:val=\"{}\"/>", esc(&p));
        }
    }
    s.push_str("<w:qFormat/>");
    if !e.caractere {
        let num = if e.codigo == format!("Heading{}", e.base) { numero_do_titulo(l.numeracao != Numeracao::Nenhuma, e.base) } else { String::new() };
        let _ = write!(s, "<w:pPr>{}</w:pPr>", paragrafo_do_estilo_xml(e, &a, &num));
    }
    let (bits, fmt) = e.caractere_do_estilo();
    let _ = write!(s, "<w:rPr>{}</w:rPr></w:style>", caractere_xml(None, bits, fmt, e.desligados));
    s
}

/// O recuo de cada nível de lista no Word, em pontos (720 twips).
pub(crate) const RECUO_DO_NIVEL: f32 = 36.0;

fn nivel_de_lista(tipo: usize, nivel: u8) -> String {
    let (fmt, texto) = TIPOS_DE_LISTA[tipo];
    let texto = if tipo == 0 { MARCADORES_PADRAO[usize::from(nivel) % 3].to_string() } else { texto.replace("%1", &format!("%{}", nivel + 1)) };
    let recuo = 720 * (i64::from(nivel) + 1);
    format!("<w:lvl w:ilvl=\"{nivel}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{recuo}\" w:hanging=\"360\"/></w:pPr></w:lvl>", esc(&texto))
}

fn numeracao_xml(nums: &[Num], titulos: Numeracao) -> String {
    let mut s = format!("{XML}<w:numbering {NS}>");
    for tipo in 0..TIPOS_DE_LISTA.len() {
        let _ = write!(s, "<w:abstractNum w:abstractNumId=\"{tipo}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>");
        for nivel in 0..9u8 {
            s.push_str(&nivel_de_lista(tipo, nivel));
        }
        s.push_str("</w:abstractNum>");
    }
    if titulos != Numeracao::Nenhuma {
        s.push_str("<w:abstractNum w:abstractNumId=\"100\"><w:multiLevelType w:val=\"multilevel\"/>");
        for n in 0..6u8 {
            let mut texto: String = (1..=n + 1).map(|i| format!("%{i}")).collect::<Vec<_>>().join(".");
            if n == 0 && titulos == Numeracao::Capitulo {
                texto = "Capítulo %1".into();
            }
            let _ = write!(s, "<w:lvl w:ilvl=\"{n}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"decimal\"/><w:pStyle w:val=\"Heading{}\"/><w:suff w:val=\"space\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/></w:lvl>", n + 1, esc(&texto));
        }
        s.push_str("</w:abstractNum>");
    }
    for (i, n) in nums.iter().enumerate() {
        let _ = write!(s, "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{}\"/>", i + 1, n.tipo);
        if n.recomeca {
            s.push_str("<w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"1\"/></w:lvlOverride>");
        }
        s.push_str("</w:num>");
    }
    if titulos != Numeracao::Nenhuma {
        let _ = write!(s, "<w:num w:numId=\"{NUM_TITULOS}\"><w:abstractNumId w:val=\"100\"/></w:num>");
    }
    s.push_str("</w:numbering>");
    s
}

fn configuracoes_xml(doc: &Documento) -> String {
    let fundo = if doc.extras.cor_pagina != 0 { "<w:displayBackgroundShape/>" } else { "" };
    let campos = if doc.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::Sumario { .. } | Conteudo::IndiceRemissivo)) { "<w:updateFields w:val=\"true\"/>" } else { "" };
    let rastrear = if doc.extras.controlar_alteracoes { "<w:trackRevisions/>" } else { "" };
    let proteger = if doc.extras.protegido { "<w:documentProtection w:edit=\"readOnly\" w:enforcement=\"1\"/>" } else { "" };
    format!("{XML}<w:settings {NS}>{fundo}{rastrear}{proteger}<w:defaultTabStop w:val=\"708\"/>{campos}<w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>")
}

fn propriedades_xml(doc: &Documento) -> String {
    format!(
        "{XML}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>{}</dc:title><dc:creator>{}</dc:creator></cp:coreProperties>",
        esc(&doc.meta.titulo),
        esc(&doc.meta.autor)
    )
}

fn raiz_rels() -> String {
    format!(
        "{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL}/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>"
    )
}

/// `footnotes.xml` ou `endnotes.xml`: os dois separadores que o Word exige (`w:id` -1 e 0) e as notas.
fn notas_xml(raiz: &str, notas: &[String]) -> String {
    let tag = &raiz[..raiz.len() - 1];
    let mut s = format!("{XML}<w:{raiz} {NS}>");
    let _ = write!(s, "<w:{tag} w:type=\"separator\" w:id=\"-1\"><w:p><w:r><w:separator/></w:r></w:p></w:{tag}>");
    let _ = write!(s, "<w:{tag} w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:{tag}>");
    for n in notas {
        s.push_str(n);
    }
    let _ = write!(s, "</w:{raiz}>");
    s
}

/// Um campo complexo (`w:fldChar`) com a instrução e o resultado já calculado.
fn campo_complexo(instr: &str, resultado: &str) -> String {
    let fim = "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>";
    let comeco = format!("<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\">{}</w:instrText></w:r>", esc(instr));
    if resultado.is_empty() {
        format!("{comeco}{fim}")
    } else {
        format!("{comeco}<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r>{}</w:r>{fim}", t(resultado))
    }
}

/// `sobrepostos`: `(caminho, ContentType)` das partes da sacola.
fn tipos_xml(extensoes: &BTreeSet<String>, notas: [bool; 2], fontes: bool, comentarios: bool, sobrepostos: &[(String, String)]) -> String {
    let wml = "application/vnd.openxmlformats-officedocument.wordprocessingml";
    let mut s = format!("{XML}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    for ext in extensoes {
        let tipo = if ext == "svg" { "image/svg+xml".to_string() } else { format!("image/{ext}") };
        let _ = write!(s, "<Default Extension=\"{ext}\" ContentType=\"{tipo}\"/>");
    }
    for (parte, tipo) in [("word/document.xml", "document.main"), ("word/styles.xml", "styles"), ("word/numbering.xml", "numbering"), ("word/settings.xml", "settings"), ("word/header1.xml", "header"), ("word/footer1.xml", "footer")] {
        let _ = write!(s, "<Override PartName=\"/{parte}\" ContentType=\"{wml}.{tipo}+xml\"/>");
    }
    for (k, (parte, tipo)) in [("word/footnotes.xml", "footnotes"), ("word/endnotes.xml", "endnotes")].into_iter().enumerate() {
        if notas[k] {
            let _ = write!(s, "<Override PartName=\"/{parte}\" ContentType=\"{wml}.{tipo}+xml\"/>");
        }
    }
    if fontes {
        s.push_str("<Override PartName=\"/customXml/itemProps1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.customXmlProperties+xml\"/>");
    }
    if comentarios {
        let _ = write!(s, "<Override PartName=\"/word/comments.xml\" ContentType=\"{wml}.comments+xml\"/><Override PartName=\"/word/commentsExtended.xml\" ContentType=\"{wml}.commentsExtended+xml\"/>");
    }
    for (parte, tipo) in sobrepostos {
        let _ = write!(s, "<Override PartName=\"/{}\" ContentType=\"{}\"/>", esc(parte), esc(tipo));
    }
    s.push_str("<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/></Types>");
    s
}
