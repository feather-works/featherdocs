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

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use featherdocs_core::layout::tamanho_base;
use featherdocs_core::modelo::{Alinhamento, Atrib, Bloco, Celula, Conteudo, Documento, EstiloDeCitacao, Estilo, Estilos, Faixas, Fmt, InicioDeSecao, Lista, Marca, NivelDeLista, Numeracao, NumeracaoDeLinhas, TipoDeRevisao, Trecho, Vertical, MaisLetra, MaisParagrafo, ALINHAMENTO_DEFINIDO, CODIGO, MATEMATICA, SERIFA};
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
/// O `abstractNumId` e o `numId` (sem reinício) da lista do Word `i` (`Estilos::listas`), e os do estilo
/// de lista `k` (o `w:styleLink` que o `w:numStyleLink` aponta).
const ABS_DO_WORD: u32 = 200;
const NUM_DO_WORD: u32 = 20000;
const ABS_DO_ESTILO: u32 = 500;
const NUM_DO_ESTILO: u32 = 30000;

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
        ("word/numbering.xml".into(), numeracao_xml(&g.nums, &doc.estilos).into_bytes()),
        ("word/settings.xml".into(), configuracoes_xml(doc).into_bytes()),
        ("docProps/core.xml".into(), propriedades_xml(doc).into_bytes()),
    ];
    for (k, (parte, raiz, _, _, f)) in faixas_do_documento(doc).into_iter().enumerate() {
        let xml = if f.blocos_valem() {
            // A faixa em blocos (tabela, vários parágrafos), com um gravador só dela: as relações
            // das imagens vão para a parte da faixa, não para o document.xml.
            let mut h = Gravador::novo(doc);
            h.figuras = 1000 * (k as u32 + 1);
            let mut corpo: String = f.blocos.iter().map(|b| h.bloco_com_controles(b)).collect();
            if !corpo.ends_with("</w:p>") {
                corpo.push_str("<w:p/>");
            }
            if !h.rels.is_empty() {
                let mut rels = format!("{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
                for (id, tipo, alvo, externo) in &h.rels {
                    let modo = if *externo { " TargetMode=\"External\"" } else { "" };
                    let _ = write!(rels, "<Relationship Id=\"{id}\" Type=\"{}\" Target=\"{}\"{modo}/>", esc(tipo), esc(alvo));
                }
                rels.push_str("</Relationships>");
                partes.push((format!("word/_rels/{parte}.rels"), rels.into_bytes()));
            }
            g.midias.append(&mut h.midias);
            g.sacola.append(&mut h.sacola);
            // A marca d'água vai em todo cabeçalho, como no Word (Design → Marca d'água).
            let (marca, ns) = if raiz == "hdr" && !doc.extras.marca.is_empty() {
                (marca_dagua_xml(&doc.extras.marca, doc.extras.marca_cor), format!("{NS} xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\""))
            } else {
                (String::new(), NS.to_string())
            };
            format!("{XML}<w:{raiz} {ns}>{marca}{corpo}</w:{raiz}>")
        } else {
            faixa_parte(&mut g, raiz, f, &parte)
        };
        let tipo = if raiz == "hdr" { "header" } else { "footer" };
        // header1 e footer1 têm o ContentType fixo em `tipos_xml`; os outros vão como os da sacola.
        if parte == "header1.xml" || parte == "footer1.xml" {
            partes.push((format!("word/{parte}"), xml.into_bytes()));
        } else {
            g.sacola.push((format!("word/{parte}"), format!("application/vnd.openxmlformats-officedocument.wordprocessingml.{tipo}+xml"), xml.into_bytes()));
        }
    }
    let mut raiz = Vec::new();
    for (parte, tipo, xml) in [("docProps/app.xml", "extended-properties", aplicativo_xml(&doc.extras.documento)), ("docProps/custom.xml", "custom-properties", personalizadas_xml(&doc.extras.documento))] {
        if let Some(xml) = xml {
            g.sacola.push((parte.into(), format!("application/vnd.openxmlformats-officedocument.{tipo}+xml"), xml.into_bytes()));
            raiz.push((format!("{REL}/{tipo}"), parte));
        }
    }
    partes.push(("_rels/.rels".into(), raiz_rels(&raiz).into_bytes()));
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

/// Os fios definidos, na ordem do esquema do Word (o que não foi definido não sai).
fn bordas_xml(b: &featherdocs_core::modelo::BordasDeTabela) -> String {
    let lados = [("top", &b.cima), ("left", &b.esquerda), ("bottom", &b.baixo), ("right", &b.direita), ("insideH", &b.dentro_h), ("insideV", &b.dentro_v)];
    let mut s = String::new();
    for (lado, fio) in lados {
        if let Some(f) = fio {
            let cor = if f.cor == 0 { "auto".to_string() } else { hex(f.cor) };
            let _ = write!(s, "<w:{lado} w:val=\"{}\" w:sz=\"{}\" w:color=\"{cor}\" w:space=\"0\"/>", esc(&f.estilo), f.espessura);
        }
    }
    s
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

/// Uma numeração concreta (`w:num`): o `abstractNumId` e o `startOverride` (nível, número).
struct Num {
    abstrata: u32,
    inicio: Option<(u8, u32)>,
}

/// O número de listas do FeatherDocs (os `abstractNumId` 0 a 9).
pub(crate) const TIPOS_NATIVOS: usize = TIPOS_DE_LISTA.len();

/// O nível `nivel` da lista do FeatherDocs `tipo`, como o gravador o põe (o leitor compara com ele para
/// saber que a lista é do FeatherDocs).
pub(crate) fn nivel_nativo(tipo: usize, nivel: u8) -> NivelDeLista {
    let (fmt, texto) = TIPOS_DE_LISTA[tipo];
    let texto = if tipo == 0 { MARCADORES_PADRAO[usize::from(nivel) % 3].to_string() } else { texto.replace("%1", &format!("%{}", nivel + 1)) };
    NivelDeLista { formato: fmt.into(), texto, inicio: 1, recuo: RECUO_DO_NIVEL * f32::from(nivel + 1), pendurado: 18.0, fonte: String::new() }
}

/// Os estilos de lista (`ListaDoWord::estilo`) distintos, na ordem: a posição é o `k` de `ABS_DO_ESTILO`.
fn estilos_de_lista(e: &Estilos) -> Vec<&str> {
    let mut v: Vec<&str> = Vec::new();
    for l in &e.listas {
        if !l.estilo.is_empty() && !v.contains(&l.estilo.as_str()) {
            v.push(&l.estilo);
        }
    }
    v
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
    /// O `numId` em uso por lista do Word (a contagem dela atravessa o documento).
    em_uso_word: HashMap<u16, u32>,
    anterior_item: bool,
    figuras: u32,
    /// Os `w:id` dos indicadores já gravados.
    indicadores: u32,
    legendas: Vec<u32>,
    /// O XML de cada nota de rodapé (0) e de fim (1), na ordem: o `w:id` é a posição + 1.
    notas: [Vec<String>; 2],
    /// Quantas sugestões já foram (o `w:id` de cada `w:ins`/`w:del`).
    revisoes: u32,
}

impl<'a> Gravador<'a> {
    fn novo(doc: &'a Documento) -> Self {
        Self { doc, rels: Vec::new(), sacola: Vec::new(), controles: 0, midias: Vec::new(), nums: Vec::new(), em_uso: [None; TIPOS_DE_LISTA.len()], em_uso_word: HashMap::new(), anterior_item: false, figuras: 0, indicadores: 0, legendas: Vec::new(), notas: [Vec::new(), Vec::new()], revisoes: 0 }
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
            // As decorações presas a um bloco saem num parágrafo de 1 pt antes dele; as de toda página
            // vão no cabeçalho (`faixa_parte`), como o timbre do Word.
            let decoracoes: Vec<_> = self.doc.extras.decoracoes.iter().filter(|d| d.bloco == Some(b.id)).cloned().collect();
            if !decoracoes.is_empty() {
                let mut desenhos = String::new();
                for d in &decoracoes {
                    desenhos.push_str(&self.ancora(d));
                }
                let _ = write!(s, "<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"20\" w:lineRule=\"exact\"/></w:pPr>{desenhos}</w:p>");
            }
            let x = self.bloco_com_controles(b);
            s.push_str(&x);
        }
        s.push_str(&FIM_DO_CONTROLE.repeat(std::mem::take(&mut self.controles)));
        s.push_str(&secao(self.doc, None));
        s.push_str("</w:body></w:document>");
        s
    }

    /// O bloco dentro dos controles de conteúdo de bloco que começam (as marcas no começo do texto)
    /// ou terminam (no fim) nele; o controle que ficou no meio do texto sai como controle da linha.
    fn bloco_com_controles(&mut self, b: &Bloco) -> String {
        let x = self.bloco_com_controles_sem_indicador(b);
        let nomes: Vec<&str> = self.doc.marcadores.iter().filter(|m| m.bloco == b.id).map(|m| m.nome.as_str()).collect();
        if nomes.is_empty() {
            return x;
        }
        // O indicador no começo do primeiro parágrafo do bloco (depois do `w:pPr`).
        let Some(p) = x.find("<w:p>").or_else(|| x.find("<w:p ")) else { return x };
        let fim_p = p + x[p..].find('>').unwrap_or(0) + 1;
        let onde = if x[fim_p..].starts_with("<w:pPr>") { x[fim_p..].find("</w:pPr>").map_or(fim_p, |k| fim_p + k + "</w:pPr>".len()) } else { fim_p };
        // O indicador cobre o texto do parágrafo (é ele que o REF mostra).
        let fim = x[onde..].find("</w:p>").map_or(x.len(), |k| onde + k);
        let (mut abre, mut fecha) = (String::new(), String::new());
        for n in nomes {
            self.indicadores += 1;
            let _ = write!(abre, "<w:bookmarkStart w:id=\"{}\" w:name=\"{}\"/>", self.indicadores, esc(n));
            let _ = write!(fecha, "<w:bookmarkEnd w:id=\"{}\"/>", self.indicadores);
        }
        format!("{}{abre}{}{fecha}{}", &x[..onde], &x[onde..fim], &x[fim..])
    }

    fn bloco_com_controles_sem_indicador(&mut self, b: &Bloco) -> String {
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
        // Como no motor (`paginacao::marcadores`), o parágrafo recuado no meio da lista não a interrompe.
        let item = matches!(b.conteudo, Conteudo::Item { .. });
        let recuado = !item && b.atrib.recuo_esq >= 18.0;
        if !recuado {
            if !item || !self.anterior_item {
                self.em_uso = [None; TIPOS_DE_LISTA.len()];
            }
            self.anterior_item = item;
        }
        match &b.conteudo {
            Conteudo::Titulo { nivel, texto } => self.titulo(b, *nivel, texto),
            Conteudo::Paragrafo { texto } => self.paragrafo(b, texto),
            Conteudo::Item { ordenado, nivel, texto } => self.item(b, *ordenado, *nivel, texto),
            Conteudo::Citacao { texto } => format!("<w:p>{}{}</w:p>", ppr(Some(&self.estilo_do_word(b).unwrap_or_else(|| "Quote".into())), &b.atrib, ""), self.trechos(texto)),
            Conteudo::Codigo { codigo, .. } => codigo_xml(codigo, &b.atrib),
            Conteudo::Tabela { cabecalho, linhas } => self.tabela(*cabecalho, linhas, b),
            Conteudo::Imagem { origem, legenda, largura, altura } => self.imagem(origem, legenda, *largura, *altura, &b.imagem),
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
                let sec = secao(self.doc, b.secao.as_deref().map(|s| (s, b.id))).replacen("<w:pgSz", &format!("<w:type w:val=\"{tipo}\"/><w:pgSz"), 1);
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
        // O parágrafo com um estilo que numera (List Number) e que não é item: o `numId` 0 tira a lista.
        let numera = self.doc.estilos.resolvido_do_bloco(b).is_some_and(|e| e.mais_paragrafo.lista != 0);
        let extra = if numera { "<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"0\"/></w:numPr>" } else { "" };
        format!("<w:p>{}{}</w:p>", self.ppr_do_bloco(estilo.as_deref(), b, extra), self.trechos(texto))
    }

    /// "Figura 1 – texto": o número é o campo `SEQ`, que o Word recalcula.
    fn legenda(&mut self, b: &Bloco, texto: &[Trecho]) -> String {
        let i = usize::from(b.atrib.legenda.max(1)) - 1;
        // Figura, tabela, equação e, depois delas, os rótulos próprios do documento (`Estilos::legendas`).
        let rotulo = ROTULOS_LEGENDA.get(i).copied().or_else(|| self.doc.estilos.legendas.get(i - 3).map(String::as_str)).unwrap_or(ROTULOS_LEGENDA[0]);
        if self.legendas.len() <= i {
            self.legendas.resize(i + 1, 0);
        }
        self.legendas[i] += 1;
        let campo = format!("<w:fldSimple w:instr=\" SEQ {rotulo} \\* ARABIC \"><w:r>{}</w:r></w:fldSimple>", t(&self.legendas[i].to_string()));
        format!("<w:p>{}<w:r>{}</w:r>{campo}<w:r>{}</w:r>{}</w:p>", ppr(Some("Caption"), &b.atrib, ""), t(&format!("{rotulo} ")), t(" – "), self.trechos(texto))
    }

    fn item(&mut self, b: &Bloco, ordenado: bool, nivel: u8, texto: &[Trecho]) -> String {
        let n = nivel.min(8);
        let a = &b.atrib;
        let do_word = usize::from(a.mais.lista).checked_sub(1).and_then(|i| self.doc.estilos.listas.get(i)).map(|l| (a.mais.lista, l));
        let (num, recuo) = match do_word {
            // A lista do Word: o `numId` dela, e um novo (com `startOverride`) onde ela recomeça.
            Some((l, lista)) => {
                let nivel_def = lista.niveis.get(usize::from(n));
                let inicio = a.mais.inicio.or_else(|| a.reiniciar.then(|| nivel_def.map_or(1, |x| x.inicio)));
                let num = match (inicio, self.em_uso_word.get(&l)) {
                    (None, Some(num)) => *num,
                    (None, None) => NUM_DO_WORD + u32::from(l) - 1,
                    (Some(v), _) => self.novo_num(ABS_DO_WORD + u32::from(l) - 1, Some((n, v))),
                };
                self.em_uso_word.insert(l, num);
                (num, nivel_def.filter(|x| x.recuo > 0.0).map_or(RECUO_DO_NIVEL * f32::from(n + 1), |x| x.recuo))
            }
            None => {
                let tipo = tipo_de_lista(ordenado, a);
                if a.reiniciar || a.mais.inicio.is_some() {
                    self.em_uso[tipo] = None;
                }
                let num = match self.em_uso[tipo] {
                    Some(num) => num,
                    None => {
                        let inicio = a.mais.inicio.map(|v| (n, v)).or_else(|| self.nums.iter().any(|x| x.abstrata == tipo as u32).then_some((0, 1)));
                        let num = self.novo_num(tipo as u32, inicio);
                        self.em_uso[tipo] = Some(num);
                        num
                    }
                };
                (num, RECUO_DO_NIVEL * f32::from(n + 1))
            }
        };
        let extra = format!("<w:numPr><w:ilvl w:val=\"{n}\"/><w:numId w:val=\"{num}\"/></w:numPr>");
        // O recuo do item soma ao do nível (`nivel_de_lista`): o `w:ind` do parágrafo troca o da numeração.
        let mut atrib = b.atrib;
        if atrib.recuo_esq != 0.0 {
            atrib.recuo_esq += recuo;
        }
        // O item com o estilo que veio do Word volta com ele; sem estilo, o do FeatherDocs (se o do Word não veio).
        let estilo = self.estilo_do_word(b).or_else(|| (!self.doc.estilos.lista.iter().any(|e| e.codigo == "ListParagraph")).then(|| "ListParagraph".to_string()));
        format!("<w:p>{}{}</w:p>", ppr(estilo.as_deref(), &atrib, &extra), self.trechos(texto))
    }

    fn novo_num(&mut self, abstrata: u32, inicio: Option<(u8, u32)>) -> u32 {
        self.nums.push(Num { abstrata, inicio });
        self.nums.len() as u32
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
        // O texto movido: de onde saiu (`w:moveFrom`, com `w:t`) e para onde foi (`w:moveTo`).
        if r.movido {
            let tag = if r.tipo == TipoDeRevisao::Insercao { "moveTo" } else { "moveFrom" };
            return format!("<w:{tag} w:id=\"{}\" w:author=\"{}\"{data}>{corrida}</w:{tag}>", self.revisoes, esc(autor));
        }
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
                // Cada QUEBRA_DE_PARAGRAFO da nota abre um parágrafo novo.
                let corpo: String = texto.iter().filter(|t| t.marca.is_none()).map(|t| corrida(t, &self.doc.estilos)).collect();
                let corpo = corpo.replace(featherdocs_core::modelo::QUEBRA_DE_PARAGRAFO, "</w:t></w:r></w:p><w:p><w:r><w:t xml:space=\"preserve\">");
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
                let (instr, resultado) = if let Some((instr, resultado)) = featherdocs_core::campo::partes_do_word(campo) {
                    (instr.to_string(), resultado.to_string())
                } else if let Some(f) = featherdocs_core::campo::formato_da_data(campo) {
                    (format!("DATE \\@ \"{f}\""), tr.rotulo.clone())
                } else {
                    let instr = match campo.as_str() {
                        "autor" => "AUTHOR",
                        "titulo" => "TITLE",
                        "pagina" => "PAGE",
                        "paginas" => "NUMPAGES",
                        _ => "FILENAME",
                    };
                    (instr.to_string(), tr.rotulo.clone())
                };
                format!("<w:fldSimple w:instr=\" {} \"><w:r>{}{}</w:r></w:fldSimple>", esc(&instr), rpr(tr, &self.doc.estilos), t(&resultado))
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
            Marca::Imagem { origem, largura, altura } => match self.blip(origem) {
                Some(blip) => format!("<w:r>{}{}</w:r>", rpr(tr, &self.doc.estilos), desenho_xml(self.figuras, emu(*largura), emu(*altura), &blip, &Default::default())),
                None => String::new(),
            },
            Marca::Controle { .. } => String::new(),
            Marca::ObjetoWord { xml, partes, blocos, .. } => {
                let mut x = self.sacola(xml, partes);
                if x.contains(featherdocs_core::modelo::TEXTO_DA_CAIXA) {
                    x = x.replace(featherdocs_core::modelo::TEXTO_DA_CAIXA, &self.blocos_da_celula(blocos));
                }
                format!("<w:r>{}{x}</w:r>", rpr(tr, &self.doc.estilos))
            }
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

    fn tabela(&mut self, cabecalho: bool, linhas: &[Vec<Celula>], b: &Bloco) -> String {
        let (mesclagens, props) = (&b.mesclagens, &b.tabela);
        let cols = linhas.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let larguras: Vec<i64> = if props.colunas.len() == cols {
            props.colunas.iter().map(|w| twips(*w).max(1)).collect()
        } else {
            vec![twips(self.doc.pagina.largura_util()) / cols as i64; cols]
        };
        // Com estilo de tabela, a grade e o cinza do cabeçalho do FeatherDocs não entram por cima dele.
        let com_estilo = !props.estilo.is_empty();
        let bordas = if props.bordas.eh_padrao() && com_estilo {
            String::new()
        } else if props.bordas.eh_padrao() {
            let borda = |lado: &str| format!("<w:{lado} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"BFBFBF\"/>");
            ["top", "left", "bottom", "right", "insideH", "insideV"].iter().map(|l| borda(l)).collect()
        } else {
            bordas_xml(&props.bordas)
        };
        let tblw = if props.largura_pct > 0.0 {
            format!("<w:tblW w:w=\"{}\" w:type=\"pct\"/>", (props.largura_pct.min(100.0) * 50.0).round() as i64)
        } else {
            format!("<w:tblW w:w=\"{}\" w:type=\"dxa\"/>", larguras.iter().sum::<i64>())
        };
        let jc = match b.atrib.alinhamento {
            Alinhamento::Centro => "<w:jc w:val=\"center\"/>",
            Alinhamento::Direita => "<w:jc w:val=\"right\"/>",
            _ => "",
        };
        let layout = if props.fixa { "<w:tblLayout w:type=\"fixed\"/>" } else { "" };
        let estilo = if com_estilo { format!("<w:tblStyle w:val=\"{}\"/>", esc(&props.estilo)) } else { String::new() };
        let bordas = if bordas.is_empty() { bordas } else { format!("<w:tblBorders>{bordas}</w:tblBorders>") };
        let look = if props.aparencia.is_empty() { String::new() } else { format!("<w:tblLook {}/>", props.aparencia) };
        let mut s = format!("<w:tbl><w:tblPr>{estilo}{tblw}{jc}{bordas}{layout}<w:tblCellMar><w:left w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"80\" w:type=\"dxa\"/></w:tblCellMar>{look}</w:tblPr><w:tblGrid>");
        for w in &larguras {
            let _ = write!(s, "<w:gridCol w:w=\"{w}\"/>");
        }
        s.push_str("</w:tblGrid>");
        for (i, linha) in linhas.iter().enumerate() {
            let topo = cabecalho && i == 0;
            let altura = props.altura(i).map(|a| format!("<w:trHeight w:val=\"{}\"{}/>", twips(a.altura), if a.exata { " w:hRule=\"exact\"" } else { "" })).unwrap_or_default();
            let cab = if topo { "<w:tblHeader/>" } else { "" };
            if altura.is_empty() && cab.is_empty() {
                s.push_str("<w:tr>");
            } else {
                let _ = write!(s, "<w:tr><w:trPr>{altura}{cab}</w:trPr>");
            }
            let mut c = 0;
            while c < cols {
                let regiao = mesclagens.iter().find(|m| m.linha == i && m.coluna == c
                    && m.colunas >= 2 && c.checked_add(m.colunas).is_some_and(|fim| fim <= cols));
                let n = regiao.map_or(1, |m| m.colunas);
                let w: i64 = larguras[c..c + n].iter().sum();
                let span = if n > 1 { format!("<w:gridSpan w:val=\"{n}\"/>") } else { String::new() };
                let encoberta = props.encoberta_por(i, c);
                let cel = props.celula(i, c).or(encoberta);
                let vmerge = if encoberta.is_some() {
                    "<w:vMerge/>"
                } else if cel.is_some_and(|p| p.linhas > 1) {
                    "<w:vMerge w:val=\"restart\"/>"
                } else {
                    ""
                };
                let tcb = cel.filter(|p| !p.bordas.eh_padrao()).map(|p| format!("<w:tcBorders>{}</w:tcBorders>", bordas_xml(&p.bordas))).unwrap_or_default();
                let fundo = match cel.map_or(0, |p| p.fundo) {
                    0 if topo && !com_estilo => "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F2F2F2\"/>".to_string(),
                    0 => String::new(),
                    f => format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>", hex(f)),
                };
                let mar = cel.and_then(|p| p.margens).map(|[a, e, b, d]| format!("<w:tcMar><w:top w:w=\"{}\" w:type=\"dxa\"/><w:left w:w=\"{}\" w:type=\"dxa\"/><w:bottom w:w=\"{}\" w:type=\"dxa\"/><w:right w:w=\"{}\" w:type=\"dxa\"/></w:tcMar>", twips(a), twips(e), twips(b), twips(d))).unwrap_or_default();
                let direcao = cel.filter(|p| !p.direcao.is_empty()).map(|p| format!("<w:textDirection w:val=\"{}\"/>", esc(&p.direcao))).unwrap_or_default();
                let valign = match cel.map_or(0, |p| p.vertical) {
                    1 => "<w:vAlign w:val=\"center\"/>",
                    2 => "<w:vAlign w:val=\"bottom\"/>",
                    _ => "",
                };
                let jc = match cel.map_or(Alinhamento::Esquerda, |p| p.alinhamento) {
                    Alinhamento::Centro => "<w:jc w:val=\"center\"/>",
                    Alinhamento::Direita => "<w:jc w:val=\"right\"/>",
                    Alinhamento::Justificado => "<w:jc w:val=\"both\"/>",
                    Alinhamento::Esquerda => "",
                };
                let texto = if encoberta.is_some() { &[][..] } else { linha.get(c).map_or(&[][..], Vec::as_slice) };
                let conteudo = match props.celula(i, c).filter(|p| encoberta.is_none() && p.blocos_valem(texto)) {
                    Some(p) => self.blocos_da_celula(&p.blocos),
                    None => format!("<w:p><w:pPr><w:spacing w:before=\"40\" w:after=\"40\"/>{jc}</w:pPr>{}</w:p>", self.trechos(texto)),
                };
                let _ = write!(s, "<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/>{span}{vmerge}{tcb}{fundo}{mar}{direcao}{valign}</w:tcPr>{conteudo}</w:tc>");
                c += n;
            }
            s.push_str("</w:tr>");
        }
        s.push_str("</w:tbl>");
        s
    }

    /// Os blocos de uma célula (vários parágrafos, lista, imagem, tabela dentro); a célula termina num parágrafo.
    fn blocos_da_celula(&mut self, blocos: &[Bloco]) -> String {
        // A lista de dentro da célula não continua nem interrompe a de fora.
        let fora = (self.em_uso, self.anterior_item);
        self.anterior_item = false;
        let mut s: String = blocos.iter().map(|b| self.bloco_com_controles(b)).collect();
        (self.em_uso, self.anterior_item) = fora;
        if !s.ends_with("</w:p>") {
            s.push_str("<w:p/>");
        }
        s
    }

    /// A decoração presa à página como `wp:anchor` (posição relativa à página, sem contornar o texto).
    fn ancora(&mut self, d: &featherdocs_core::modelo::Decoracao) -> String {
        let Some(blip) = self.blip(&d.origem) else { return String::new() };
        let n = self.figuras;
        let (cx, cy) = (emu(d.largura), emu(d.altura));
        format!(
            "<w:r><w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"{n}\" behindDoc=\"{}\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"page\"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"page\"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:wrapNone/><wp:docPr id=\"{n}\" name=\"Decoração {n}\"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"Decoração {n}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip {blip}/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>",
            u8::from(d.atras), emu(d.x), emu(d.y),
        )
    }

    /// O `r:embed` (a imagem vai para `media/`) ou o `r:link` da imagem; `None` = origem que o Word não abre.
    fn blip(&mut self, origem: &str) -> Option<String> {
        if let Some((tipo, bytes)) = base64::de_data_url(origem) {
            self.figuras += 1;
            let nome = format!("imagem{}.{}", self.figuras, extensao_da_imagem(&tipo));
            self.midias.push((nome.clone(), bytes));
            Some(format!("r:embed=\"{}\"", self.rel("image", format!("media/{nome}"), false)))
        } else if origem.starts_with("http://") || origem.starts_with("https://") {
            self.figuras += 1;
            Some(format!("r:link=\"{}\"", self.rel("image", origem.to_string(), true)))
        } else {
            None
        }
    }

    fn imagem(&mut self, origem: &str, legenda: &[Trecho], largura: f32, altura: f32, props: &featherdocs_core::modelo::PropsImagem) -> String {
        let (w, h) = if largura > 0.0 && altura > 0.0 { (largura, altura) } else { (300.0, 200.0) };
        let util = self.doc.pagina.largura_util();
        // A flutuante pode passar da margem (é o que o Word faz); a em linha encolhe para caber.
        let (w, h) = if w > util && props.flutuante.is_none() { (util, h * util / w) } else { (w, h) };
        let Some(blip) = self.blip(origem) else {
            return format!("<w:p><w:r>{}</w:r></w:p>", t(&format!("[imagem: {origem}]")));
        };
        let n = self.figuras;
        let (cx, cy) = (emu(w), emu(h));
        let desenho = desenho_xml(n, cx, cy, &blip, props);
        let mut s = format!("<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r>{desenho}</w:r></w:p>");
        if !legenda.is_empty() {
            let _ = write!(s, "<w:p><w:pPr><w:pStyle w:val=\"FDLegendaImagem\"/></w:pPr>{}</w:p>", self.trechos(legenda));
        }
        s
    }

    fn relacoes_xml(&self, notas: [bool; 2], fontes: bool, comentarios: bool) -> String {
        let mut s = format!("{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
        for (id, tipo, alvo) in [("rIdEstilos", "styles", "styles.xml"), ("rIdNumeracao", "numbering", "numbering.xml"), ("rIdConfig", "settings", "settings.xml")] {
            let _ = write!(s, "<Relationship Id=\"{id}\" Type=\"{REL}/{tipo}\" Target=\"{alvo}\"/>");
        }
        for (parte, raiz, id, _, _) in faixas_do_documento(self.doc) {
            let tipo = if raiz == "hdr" { "header" } else { "footer" };
            let _ = write!(s, "<Relationship Id=\"{id}\" Type=\"{REL}/{tipo}\" Target=\"{parte}\"/>");
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
/// As partes de cabeçalho e rodapé que o documento grava: (parte, raiz, `r:id`, `w:type`, faixa).
/// As do documento e, depois, as das seções com faixas próprias (`headerS<id do bloco>.xml`).
fn faixas_do_documento(doc: &Documento) -> Vec<(String, &'static str, String, &'static str, &Faixas)> {
    let x = &doc.extras;
    let mut v: Vec<(String, &'static str, String, &'static str, &Faixas)> = vec![("header1.xml".into(), "hdr", "rIdCabecalho".into(), "default", &x.cabecalho), ("footer1.xml".into(), "ftr", "rIdRodape".into(), "default", &x.rodape)];
    if x.primeira_diferente {
        v.extend([("header2.xml".into(), "hdr", "rIdCabecalho2".into(), "first", &x.cabecalho_primeira), ("footer2.xml".into(), "ftr", "rIdRodape2".into(), "first", &x.rodape_primeira)]);
    }
    if x.documento.pares_diferentes {
        v.extend([("header3.xml".into(), "hdr", "rIdCabecalho3".into(), "even", &x.documento.cabecalho_par), ("footer3.xml".into(), "ftr", "rIdRodape3".into(), "even", &x.documento.rodape_par)]);
    }
    for b in &doc.blocos {
        if let Some(s) = &b.secao {
            v.push((format!("headerS{}.xml", b.id), "hdr", format!("rIdCabecalhoS{}", b.id), "default", &s.cabecalho));
            v.push((format!("footerS{}.xml", b.id), "ftr", format!("rIdRodapeS{}", b.id), "default", &s.rodape));
        }
    }
    v
}

/// O `w:sectPr` do documento ou, com `propria` (a seção e o id do bloco da quebra), o de uma seção
/// com papel e faixas próprios.
fn secao(doc: &Documento, propria: Option<(&featherdocs_core::modelo::Secao, u64)>) -> String {
    let p = propria.map_or(&doc.pagina, |(s, _)| &s.pagina);
    let x = &doc.extras;
    let mut s = String::from("<w:sectPr>");
    // As faixas da seção: as de sempre trocadas pelas dela; os cabeçalhos antes dos rodapés, como no esquema.
    let da_secao = |rid: &str| match propria {
        Some((_, id)) => rid == format!("rIdCabecalhoS{id}") || rid == format!("rIdRodapeS{id}"),
        None => rid == "rIdCabecalho" || rid == "rIdRodape",
    };
    let mut faixas: Vec<_> = faixas_do_documento(doc).into_iter().filter(|f| f.3 != "default" || da_secao(&f.2)).collect();
    faixas.sort_by_key(|f| f.1 == "ftr");
    for (_, raiz, id, tipo, _) in faixas {
        let elemento = if raiz == "hdr" { "headerReference" } else { "footerReference" };
        let _ = write!(s, "<w:{elemento} w:type=\"{tipo}\" r:id=\"{id}\"/>");
    }
    // O formato e o recomeço das notas (o de sempre do Word: 1, 2, 3 no rodapé e i, ii, iii no fim).
    const FORMATOS: [&str; 4] = ["decimal", "lowerLetter", "lowerRoman", "chicago"];
    if x.formato_notas != 0 || x.documento.notas_por_pagina {
        s.push_str("<w:footnotePr>");
        if x.formato_notas != 0 {
            let _ = write!(s, "<w:numFmt w:val=\"{}\"/>", FORMATOS[usize::from(x.formato_notas.min(3))]);
        }
        if x.documento.notas_por_pagina {
            s.push_str("<w:numRestart w:val=\"eachPage\"/>");
        }
        s.push_str("</w:footnotePr>");
    }
    if x.formato_notas_fim != 2 {
        let _ = write!(s, "<w:endnotePr><w:numFmt w:val=\"{}\"/></w:endnotePr>", FORMATOS[usize::from(x.formato_notas_fim.min(3))]);
    }
    let paisagem = if p.largura > p.altura { " w:orient=\"landscape\"" } else { "" };
    let _ = write!(s, "<w:pgSz w:w=\"{}\" w:h=\"{}\"{paisagem}/>", twips(p.largura), twips(p.altura));
    // A faixa fica onde a tela a desenha: a linha de base na metade da margem (`Quadro::cabecalho_e_rodape`).
    let m = &x.documento;
    let auto = featherdocs_core::modelo::distancia_automatica;
    let cabecalho = if m.distancia_cabecalho > 0.0 { m.distancia_cabecalho } else { auto(p, propria.map_or(&x.cabecalho, |(s, _)| &s.cabecalho), true) };
    let rodape = if m.distancia_rodape > 0.0 { m.distancia_rodape } else { auto(p, propria.map_or(&x.rodape, |(s, _)| &s.rodape), false) };
    let _ = write!(s, "<w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"{}\"/>", twips(p.margem_sup), twips(p.margem_dir), twips(p.margem_inf), twips(p.margem_esq), twips(cabecalho), twips(rodape), twips(m.medianiz));
    if x.borda_cor != 0 {
        let sz = ((x.borda_espessura.max(0.25) * 8.0).round() as i64).clamp(2, 96);
        let espaco = (x.borda_distancia.round() as i64).clamp(0, 31);
        let traco = if x.documento.borda_estilo.is_empty() { "single".to_string() } else { esc(&x.documento.borda_estilo) };
        let lado = |l: &str| format!("<w:{l} w:val=\"{traco}\" w:sz=\"{sz}\" w:space=\"{espaco}\" w:color=\"{}\"/>", hex(x.borda_cor));
        let _ = write!(s, "<w:pgBorders w:offsetFrom=\"page\">{}{}{}{}</w:pgBorders>", lado("top"), lado("left"), lado("bottom"), lado("right"));
    }
    let passo = x.documento.passo_linhas.max(1);
    match x.numerar_linhas {
        NumeracaoDeLinhas::Nenhuma => {}
        NumeracaoDeLinhas::Continua => { let _ = write!(s, "<w:lnNumType w:countBy=\"{passo}\" w:restart=\"continuous\"/>"); }
        NumeracaoDeLinhas::PorPagina => { let _ = write!(s, "<w:lnNumType w:countBy=\"{passo}\" w:restart=\"newPage\"/>"); }
    }
    let inicio = if x.pagina_inicial != 1 { format!(" w:start=\"{}\"", x.pagina_inicial) } else { String::new() };
    let formato = if x.documento.formato_pagina.is_empty() { String::new() } else { format!(" w:fmt=\"{}\"", esc(&x.documento.formato_pagina)) };
    if !inicio.is_empty() || !formato.is_empty() {
        let _ = write!(s, "<w:pgNumType{formato}{inicio}/>");
    }
    if x.documento.colunas > 1 {
        let espaco = if x.documento.espaco_colunas > 0.0 { twips(x.documento.espaco_colunas) } else { 720 };
        let _ = write!(s, "<w:cols w:num=\"{}\" w:space=\"{espaco}\"/>", x.documento.colunas);
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

/// A parte `nome` (`header1.xml`) do cabeçalho ou rodapé: o logo vai para as mídias, com as
/// relações da parte; o cabeçalho leva também a marca d'água e o timbre.
fn faixa_parte(g: &mut Gravador, raiz: &str, f: &Faixas, nome: &str) -> String {
    // As imagens da parte entram nas relações dela, não nas do documento.
    let do_documento = std::mem::take(&mut g.rels);
    let logo = (f.imagem_largura > 0.0 && f.imagem_altura > 0.0).then(|| g.blip(&f.imagem)).flatten().map(|blip| {
        let (cx, cy) = (emu(f.imagem_largura), emu(f.imagem_altura));
        format!("<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{}\" name=\"Logo\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"0\" name=\"Logo\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip {blip}/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>", 9000 + g.figuras)
    });
    // O timbre: as figuras de toda página (sem bloco), presas à página, num parágrafo de 1 pt.
    let mut timbre = String::new();
    if raiz == "hdr" {
        let todas: Vec<_> = g.doc.extras.decoracoes.iter().filter(|d| d.bloco.is_none()).cloned().collect();
        for d in &todas {
            timbre.push_str(&g.ancora(d));
        }
    }
    let x = &g.doc.extras;
    let mut antes = if raiz == "hdr" && !x.marca.is_empty() { marca_dagua_xml(&x.marca, x.marca_cor) } else { String::new() };
    if !timbre.is_empty() {
        let _ = write!(antes, "<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"20\" w:lineRule=\"exact\"/></w:pPr>{timbre}</w:p>");
    }
    let da_parte = std::mem::replace(&mut g.rels, do_documento);
    if !da_parte.is_empty() {
        let mut rels = format!("{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
        for (id, tipo, alvo, externo) in da_parte {
            let modo = if externo { " TargetMode=\"External\"" } else { "" };
            let _ = write!(rels, "<Relationship Id=\"{id}\" Type=\"{tipo}\" Target=\"{}\"{modo}/>", esc(&alvo));
        }
        rels.push_str("</Relationships>");
        g.sacola.push((format!("word/_rels/{nome}.rels"), String::new(), rels.into_bytes()));
    }
    let vml = raiz == "hdr" && !x.marca.is_empty();
    faixa_xml(raiz, f, &g.doc.pagina, logo.as_deref(), (!antes.is_empty()).then_some(antes.as_str()), vml)
}

/// A marca d'água como o Word grava: o WordArt VML (`_x0000_t136`) no cabeçalho, girado e atrás do texto.
fn marca_dagua_xml(texto: &str, cor: u32) -> String {
    let opacidade = if cor & 0xFF == 0 { ".5".to_string() } else { format!("{:.3}", (cor & 0xFF) as f32 / 255.0) };
    let cor = if cor == 0 { "silver".to_string() } else { format!("#{}", hex(cor)) };
    format!(concat!(
        "<w:p><w:r><w:pict><v:shapetype id=\"_x0000_t136\" coordsize=\"21600,21600\" o:spt=\"136\" adj=\"10800\" path=\"m@7,l@8,m@5,21600l@6,21600e\">",
        "<v:path textpathok=\"t\" o:connecttype=\"custom\"/><v:textpath on=\"t\" fitshape=\"t\"/></v:shapetype>",
        "<v:shape id=\"PowerPlusWaterMarkObject1\" o:spid=\"_x0000_s2049\" type=\"#_x0000_t136\" style=\"position:absolute;margin-left:0;margin-top:0;width:468pt;height:117pt;rotation:315;z-index:-251655168;",
        "mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin\" o:allowincell=\"f\" fillcolor=\"{}\" stroked=\"f\">",
        "<v:fill opacity=\"{}\"/><v:textpath style=\"font-family:&quot;Arial&quot;;font-size:1pt\" string=\"{}\"/></v:shape></w:pict></w:r></w:p>"), cor, opacidade, esc(texto))
}

/// Cabeçalho ou rodapé: esquerda, centro e direita separados por tabulação (centro e direita), na
/// letra da faixa, com o fio entre ela e o texto; `logo` e `marca` são parágrafos já prontos.
fn faixa_xml(raiz: &str, f: &Faixas, p: &featherdocs_core::modelo::ConfigPagina, logo: Option<&str>, marca: Option<&str>, vml: bool) -> String {
    let util = twips(p.largura_util());
    let ns = if vml { format!("{NS} xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\"") } else { NS.to_string() };
    let mut s = format!("{XML}<w:{raiz} {ns}>{}{}", marca.unwrap_or(""), logo.unwrap_or(""));
    if f.esquerda.is_empty() && f.centro.is_empty() && f.direita.is_empty() {
        if logo.is_none() && marca.is_none() {
            s.push_str("<w:p/>");
        }
        let _ = write!(s, "</w:{raiz}>");
        return s;
    }
    let fio = if f.fio > 0.0 {
        let lado = if raiz == "hdr" { "bottom" } else { "top" };
        format!("<w:pBdr><w:{lado} w:val=\"single\" w:sz=\"{}\" w:space=\"1\" w:color=\"auto\"/></w:pBdr>", ((f.fio * 8.0).round() as i64).clamp(2, 96))
    } else {
        String::new()
    };
    // Sem o espaço depois do Normal: a faixa fica na altura em que a tela a desenha.
    let _ = write!(s, "<w:p><w:pPr>{fio}<w:tabs><w:tab w:val=\"center\" w:pos=\"{}\"/><w:tab w:val=\"right\" w:pos=\"{util}\"/></w:tabs><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>", util / 2);
    let mut rpr = String::new();
    if f.estilo & SERIFA != 0 {
        let _ = write!(rpr, "<w:rFonts w:ascii=\"{FONTE_SERIFA}\" w:hAnsi=\"{FONTE_SERIFA}\" w:cs=\"{FONTE_SERIFA}\"/>");
    }
    if f.estilo & featherdocs_core::modelo::NEGRITO != 0 {
        rpr.push_str("<w:b/>");
    }
    if f.estilo & featherdocs_core::modelo::ITALICO != 0 {
        rpr.push_str("<w:i/>");
    }
    if f.cor != 0 {
        let _ = write!(rpr, "<w:color w:val=\"{}\"/>", hex(f.cor));
    }
    if f.tamanho > 0.0 {
        let _ = write!(rpr, "<w:sz w:val=\"{}\"/>", (f.tamanho * 2.0).round() as i64);
    }
    let rpr = if rpr.is_empty() { rpr } else { format!("<w:rPr>{rpr}</w:rPr>") };
    for (i, texto) in [&f.esquerda, &f.centro, &f.direita].into_iter().enumerate() {
        if i > 0 {
            let _ = write!(s, "<w:r>{rpr}<w:tab/></w:r>");
        }
        s.push_str(&texto_com_campos(texto, &rpr));
    }
    let _ = write!(s, "</w:p></w:{raiz}>");
    s
}

fn extensao_da_imagem(tipo: &str) -> &'static str {
    match tipo {
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        _ => "png",
    }
}

/// A imagem do Word (`w:drawing`, em linha ou com `wp:anchor`), com o tamanho em EMU, o `blip`
/// (`r:embed=…`) e o texto alternativo, o recorte, a rotação e a posição de `props`.
fn desenho_xml(n: u32, cx: i64, cy: i64, blip: &str, props: &featherdocs_core::modelo::PropsImagem) -> String {
    let mut doc_pr = format!("<wp:docPr id=\"{n}\" name=\"Imagem {n}\"");
    for (nome, v) in [("descr", &props.descricao), ("title", &props.titulo)] {
        if !v.is_empty() {
            let _ = write!(doc_pr, " {nome}=\"{}\"", esc(v));
        }
    }
    doc_pr.push_str("/>");
    let recorte = if props.recorte == [0.0; 4] {
        String::new()
    } else {
        let [l, t, r, b] = props.recorte.map(|v| (v * 1000.0).round() as i64);
        format!("<a:srcRect l=\"{l}\" t=\"{t}\" r=\"{r}\" b=\"{b}\"/>")
    };
    let mut xfrm = String::new();
    if props.rotacao != 0.0 {
        let _ = write!(xfrm, " rot=\"{}\"", (props.rotacao * 60000.0).round() as i64);
    }
    if props.espelhar_h {
        xfrm.push_str(" flipH=\"1\"");
    }
    if props.espelhar_v {
        xfrm.push_str(" flipV=\"1\"");
    }
    let grafico = format!("<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"Imagem {n}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip {blip}/>{recorte}<a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm{xfrm}><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>");
    let Some(f) = &props.flutuante else {
        return format!("<w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/>{doc_pr}{grafico}</wp:inline></w:drawing>");
    };
    let posicao = |eixo: &str, p: &featherdocs_core::modelo::PosicaoFlutuante, padrao: &str| {
        let de = if p.relativo.is_empty() { padrao } else { &p.relativo };
        let dentro = if p.alinhar.is_empty() { format!("<wp:posOffset>{}</wp:posOffset>", emu(p.deslocamento)) } else { format!("<wp:align>{}</wp:align>", esc(&p.alinhar)) };
        format!("<wp:position{eixo} relativeFrom=\"{}\">{dentro}</wp:position{eixo}>", esc(de))
    };
    let lado = if f.lado.is_empty() { "bothSides" } else { &f.lado };
    // O contorno justo e o atravessado pedem o polígono no esquema: o retângulo da imagem.
    const POLIGONO: &str = "<wp:wrapPolygon edited=\"0\"><wp:start x=\"0\" y=\"0\"/><wp:lineTo x=\"0\" y=\"21600\"/><wp:lineTo x=\"21600\" y=\"21600\"/><wp:lineTo x=\"21600\" y=\"0\"/><wp:lineTo x=\"0\" y=\"0\"/></wp:wrapPolygon>";
    let quebra = match f.quebra.as_str() {
        "tight" => format!("<wp:wrapTight wrapText=\"{}\">{POLIGONO}</wp:wrapTight>", esc(lado)),
        "through" => format!("<wp:wrapThrough wrapText=\"{}\">{POLIGONO}</wp:wrapThrough>", esc(lado)),
        "topAndBottom" => "<wp:wrapTopAndBottom/>".into(),
        "none" => "<wp:wrapNone/>".into(),
        _ => format!("<wp:wrapSquare wrapText=\"{}\"/>", esc(lado)),
    };
    let [dt, db, dl, dr] = f.distancias.map(emu);
    format!(
        "<w:drawing><wp:anchor distT=\"{dt}\" distB=\"{db}\" distL=\"{dl}\" distR=\"{dr}\" simplePos=\"0\" relativeHeight=\"{}\" behindDoc=\"{}\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/>{}{}<wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>{quebra}{doc_pr}{grafico}</wp:anchor></w:drawing>",
        251_658_240 + n,
        u8::from(f.atras),
        posicao("H", &f.horizontal, "column"),
        posicao("V", &f.vertical, "paragraph"),
    )
}

/// `{pagina}`, `{total}`, `{pagina_romano}`, `{titulo}` e `{autor}` viram campos do Word.
pub(crate) const CAMPOS: [(&str, &str); 5] = [("{pagina}", "PAGE"), ("{total}", "NUMPAGES"), ("{pagina_romano}", "PAGE \\* roman"), ("{titulo}", "TITLE"), ("{autor}", "AUTHOR")];

fn texto_com_campos(texto: &str, rpr: &str) -> String {
    let mut s = String::new();
    let mut resto = texto;
    while !resto.is_empty() {
        let proximo = CAMPOS.iter().filter_map(|(marca, instr)| resto.find(marca).map(|i| (i, *marca, *instr))).min_by_key(|(i, ..)| *i);
        let Some((i, marca, instr)) = proximo else {
            let _ = write!(s, "<w:r>{rpr}{}</w:r>", t(resto));
            break;
        };
        if i > 0 {
            let _ = write!(s, "<w:r>{rpr}{}</w:r>", t(&resto[..i]));
        }
        let _ = write!(s, "<w:fldSimple w:instr=\" {} \"><w:r>{rpr}{}</w:r></w:fldSimple>", esc(instr), t("1"));
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
    // O idioma do documento (o do padrão, se o estilo tiver), ou o português de sempre.
    let idioma = if doc.extras.documento.idioma.is_empty() { "pt-BR".to_string() } else { esc(&doc.extras.documento.idioma) };
    let lang = if rpr_padrao.contains("<w:lang ") { String::new() } else { format!("<w:lang w:val=\"{idioma}\"/>") };
    let mut s = format!("{XML}<w:styles {NS}><w:docDefaults><w:rPrDefault><w:rPr>{rpr_padrao}{lang}</w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>{ppr_padrao}</w:pPr></w:pPrDefault></w:docDefaults>");
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
    // shortcut: o estilo de lista volta com o `styleId` como nome (o nome do Word não se guarda); guardar se alguém sentir falta.
    for (k, nome) in estilos_de_lista(&doc.estilos).iter().enumerate() {
        let _ = write!(s, "<w:style w:type=\"numbering\" w:customStyle=\"1\" w:styleId=\"{0}\"><w:name w:val=\"{0}\"/><w:pPr><w:numPr><w:numId w:val=\"{1}\"/></w:numPr></w:pPr></w:style>", esc(nome), NUM_DO_ESTILO + k as u32);
    }
    s.push_str(&doc.estilos.de_tabela);
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
        let num = if e.codigo == format!("Heading{}", e.base) {
            numero_do_titulo(l.numeracao != Numeracao::Nenhuma, e.base)
        } else if e.mais_paragrafo.lista != 0 && usize::from(e.mais_paragrafo.lista) <= l.listas.len() {
            format!("<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{}\"/></w:numPr>", e.mais_paragrafo.nivel_lista, NUM_DO_WORD + u32::from(e.mais_paragrafo.lista) - 1)
        } else {
            String::new()
        };
        let _ = write!(s, "<w:pPr>{}</w:pPr>", paragrafo_do_estilo_xml(e, &a, &num));
    }
    let (bits, fmt) = e.caractere_do_estilo();
    let _ = write!(s, "<w:rPr>{}</w:rPr></w:style>", caractere_xml(None, bits, fmt, e.desligados));
    s
}

/// O recuo de cada nível de lista no Word, em pontos (720 twips).
pub(crate) const RECUO_DO_NIVEL: f32 = 36.0;

fn nivel_xml(nivel: usize, l: &NivelDeLista) -> String {
    let fonte = if l.fonte.is_empty() { String::new() } else { format!("<w:rPr><w:rFonts w:ascii=\"{0}\" w:hAnsi=\"{0}\" w:hint=\"default\"/></w:rPr>", esc(&l.fonte)) };
    let tw = |pt: f32| (pt * 20.0).round() as i64;
    format!(
        "<w:lvl w:ilvl=\"{nivel}\"><w:start w:val=\"{}\"/><w:numFmt w:val=\"{}\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"{}\"/></w:pPr>{fonte}</w:lvl>",
        l.inicio,
        esc(if l.formato.is_empty() { "none" } else { &l.formato }),
        esc(&l.texto),
        tw(l.recuo),
        tw(l.pendurado)
    )
}

/// Um `w:abstractNum` com estes níveis e o que vem antes deles (`w:styleLink`, `w:numStyleLink`).
fn abstrata_xml(id: u32, antes: &str, niveis: &[NivelDeLista]) -> String {
    let mut s = format!("<w:abstractNum w:abstractNumId=\"{id}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>{antes}");
    // O nível que faltava no arquivo (vazio no modelo) continua faltando.
    for (n, l) in niveis.iter().enumerate().take(9).filter(|(_, l)| **l != NivelDeLista::default()) {
        s.push_str(&nivel_xml(n, l));
    }
    s.push_str("</w:abstractNum>");
    s
}

fn numeracao_xml(nums: &[Num], estilos: &Estilos) -> String {
    let titulos = estilos.numeracao;
    let mut s = format!("{XML}<w:numbering {NS}>");
    for tipo in 0..TIPOS_DE_LISTA.len() {
        let niveis: Vec<NivelDeLista> = (0..9u8).map(|n| nivel_nativo(tipo, n)).collect();
        s.push_str(&abstrata_xml(tipo as u32, "", &niveis));
    }
    // As listas do Word; a do estilo de lista só aponta o estilo, que aponta a definição (com os níveis).
    let de_estilo = estilos_de_lista(estilos);
    for (i, l) in estilos.listas.iter().enumerate() {
        let id = ABS_DO_WORD + i as u32;
        if l.estilo.is_empty() {
            s.push_str(&abstrata_xml(id, "", &l.niveis));
        } else {
            let _ = write!(s, "<w:abstractNum w:abstractNumId=\"{id}\"><w:multiLevelType w:val=\"hybridMultilevel\"/><w:numStyleLink w:val=\"{}\"/></w:abstractNum>", esc(&l.estilo));
        }
    }
    for (k, nome) in de_estilo.iter().enumerate() {
        let niveis = estilos.listas.iter().find(|l| l.estilo == *nome).map_or(&[][..], |l| &l.niveis);
        s.push_str(&abstrata_xml(ABS_DO_ESTILO + k as u32, &format!("<w:styleLink w:val=\"{}\"/>", esc(nome)), niveis));
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
        let _ = write!(s, "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{}\"/>", i + 1, n.abstrata);
        if let Some((nivel, v)) = n.inicio {
            let _ = write!(s, "<w:lvlOverride w:ilvl=\"{nivel}\"><w:startOverride w:val=\"{v}\"/></w:lvlOverride>");
        }
        s.push_str("</w:num>");
    }
    for i in 0..estilos.listas.len() as u32 {
        let _ = write!(s, "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{}\"/></w:num>", NUM_DO_WORD + i, ABS_DO_WORD + i);
    }
    for k in 0..de_estilo.len() as u32 {
        let _ = write!(s, "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{}\"/></w:num>", NUM_DO_ESTILO + k, ABS_DO_ESTILO + k);
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
    let m = &doc.extras.documento;
    let tab = if m.tabulacao > 0.0 { twips(m.tabulacao) } else { 708 };
    let hifenizar = if m.hifenizar { "<w:autoHyphenation/>" } else { "" };
    let pares = if m.pares_diferentes { "<w:evenAndOddHeaders/>" } else { "" };
    format!("{XML}<w:settings {NS}>{fundo}{rastrear}{proteger}<w:defaultTabStop w:val=\"{tab}\"/>{hifenizar}{pares}{campos}<w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>")
}

fn propriedades_xml(doc: &Documento) -> String {
    let m = &doc.extras.documento;
    let mut s = format!(
        "{XML}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>{}</dc:title><dc:creator>{}</dc:creator>",
        esc(&doc.meta.titulo),
        esc(&doc.meta.autor)
    );
    for (tag, v) in [("dc:subject", &m.assunto), ("cp:keywords", &m.palavras_chave), ("dc:description", &m.descricao), ("cp:category", &m.categoria)] {
        if !v.is_empty() {
            let _ = write!(s, "<{tag}>{}</{tag}>", esc(v));
        }
    }
    for (tag, v) in [("dcterms:created", &m.criado), ("dcterms:modified", &m.modificado)] {
        if !v.is_empty() {
            let _ = write!(s, "<{tag} xsi:type=\"dcterms:W3CDTF\">{}</{tag}>", esc(v));
        }
    }
    s.push_str("</cp:coreProperties>");
    s
}

/// `docProps/app.xml` (empresa, gerente), se o documento tem.
fn aplicativo_xml(m: &featherdocs_core::modelo::MaisDocumento) -> Option<String> {
    if m.empresa.is_empty() && m.gerente.is_empty() {
        return None;
    }
    let mut s = format!("{XML}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>FeatherDocs</Application>");
    for (tag, v) in [("Manager", &m.gerente), ("Company", &m.empresa)] {
        if !v.is_empty() {
            let _ = write!(s, "<{tag}>{}</{tag}>", esc(v));
        }
    }
    s.push_str("</Properties>");
    Some(s)
}

/// `docProps/custom.xml` (as propriedades personalizadas, todas texto), se o documento tem.
fn personalizadas_xml(m: &featherdocs_core::modelo::MaisDocumento) -> Option<String> {
    if m.personalizadas.is_empty() {
        return None;
    }
    let mut s = format!("{XML}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">");
    for (i, (nome, valor)) in m.personalizadas.iter().enumerate() {
        let _ = write!(s, "<property fmtid=\"{{D5CDD505-2E9C-101B-9397-08002B2CF9AE}}\" pid=\"{}\" name=\"{}\"><vt:lpwstr>{}</vt:lpwstr></property>", i + 2, esc(nome), esc(valor));
    }
    s.push_str("</Properties>");
    Some(s)
}

/// As relações do pacote; `mais` = (tipo, parte) de `app.xml`/`custom.xml`.
fn raiz_rels(mais: &[(String, &str)]) -> String {
    let mut s = format!(
        "{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL}/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>"
    );
    for (i, (tipo, parte)) in mais.iter().enumerate() {
        let _ = write!(s, "<Relationship Id=\"rId{}\" Type=\"{tipo}\" Target=\"{parte}\"/>", i + 3);
    }
    s.push_str("</Relationships>");
    s
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
