//! Exportar para texto legível sem depender de nenhum programa da suíte:
//! Markdown e LaTeX.

use featherdocs_core::edicao::tabela_para_fonte;
use featherdocs_core::inline::{serializar, texto_puro};
use featherdocs_core::modelo::{Alinhamento, Bloco, Conteudo, Documento, EstiloDeCitacao, InicioDeSecao, Marca, Numeracao, Trecho, CODIGO, ITALICO, MATEMATICA, NEGRITO, SUBLINHADO, TACHADO};

/// As linhas `[de, ate)` (contadas de 0) que um bloco ocupa no texto exportado.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct LinhasDoBloco {
    pub id: u64,
    pub de: usize,
    pub ate: usize,
}

/// O texto exportado e onde cada bloco foi parar nele (para o modo dividido ligar código e página).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Mapa {
    pub texto: String,
    pub blocos: Vec<LinhasDoBloco>,
}

impl Mapa {
    fn linhas(&self) -> usize {
        self.texto.matches('\n').count()
    }

    fn solto(&mut self, s: &str) {
        self.texto.push_str(s);
    }

    fn bloco(&mut self, id: u64, s: &str) {
        let de = self.linhas();
        self.texto.push_str(s);
        self.blocos.push(LinhasDoBloco { id, de, ate: de + s.matches('\n').count() + 1 });
    }
}

/// O documento como Markdown. Sumário vira `[TOC]`; fórmula, `$$ … $$`.
/// A equação já é um ambiente de destaque inteiro (`equation`, `align`…, com ou sem estrela): no
/// LaTeX e no Markdown ela sai crua, sem `\[…\]`/`$$…$$` em volta.
fn ambiente_de_exibicao(latex: &str) -> bool {
    let Some(r) = latex.trim_start().strip_prefix("\\begin{") else { return false };
    let nome = r.split('}').next().unwrap_or("").trim_end_matches('*');
    matches!(nome, "equation" | "align" | "gather" | "multline" | "eqnarray" | "flalign" | "alignat" | "xalignat" | "dmath" | "dgroup" | "dseries" | "empheq")
}

pub fn markdown(doc: &Documento) -> String {
    markdown_com_mapa(doc).texto
}

/// Uma cópia do documento com as notas numeradas em ordem (1, 2, 3… as de rodapé e as de fim, cada
/// uma na sua conta): os rótulos que o Markdown usa nas chamadas e nas definições.
fn com_notas_numeradas(doc: &Documento) -> Documento {
    let mut d = doc.clone();
    let (mut rodape, mut fim) = (0, 0);
    for b in &mut d.blocos {
        for t in b.conteudo.textos_mut() {
            for tr in t.iter_mut() {
                if let Some(Marca::Nota { fim: f, .. }) = tr.marca.as_deref() {
                    let n = if *f { &mut fim } else { &mut rodape };
                    *n += 1;
                    tr.rotulo = n.to_string();
                }
            }
        }
    }
    d
}

/// As definições das notas no fim do Markdown: `[^1]: texto`.
fn definicoes_das_notas(doc: &Documento) -> String {
    let mut s = String::new();
    for b in &doc.blocos {
        for t in b.conteudo.textos() {
            for tr in t {
                if let Some(Marca::Nota { fim, texto }) = tr.marca.as_deref() {
                    s.push_str(&format!("[^{}{}]: {}\n", if *fim { "fim-" } else { "" }, tr.rotulo, serializar(texto)));
                }
            }
        }
    }
    s
}

/// O documento como ficaria aceitando todas as sugestões (o Markdown e o LaTeX não têm controle de
/// alterações: sai o texto final). Sem sugestões, nada muda.
fn sem_sugestoes(doc: &Documento) -> std::borrow::Cow<'_, Documento> {
    let tem = doc.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().any(|t| t.rev.is_some());
    if !tem {
        return std::borrow::Cow::Borrowed(doc);
    }
    let mut d = doc.clone();
    for t in d.blocos.iter_mut().flat_map(|b| b.conteudo.textos_mut()) {
        t.retain(|x| !matches!(x.rev.as_deref(), Some(r) if r.tipo == featherdocs_core::modelo::TipoDeRevisao::Remocao));
        for x in t.iter_mut() {
            x.rev = None;
        }
        featherdocs_core::texto::normalizar(t);
    }
    std::borrow::Cow::Owned(d)
}

/// Como `markdown`, com as linhas de cada bloco.
pub fn markdown_com_mapa(doc: &Documento) -> Mapa {
    let doc = &com_notas_numeradas(&sem_sugestoes(doc));
    let mut m = Mapa::default();
    let mut anterior_item = false;
    for b in &doc.blocos {
        let item = matches!(b.conteudo, Conteudo::Item { .. });
        if !m.texto.is_empty() && !(item && anterior_item) {
            m.solto("\n");
        }
        anterior_item = item;
        m.bloco(b.id, &bloco_markdown(&b.conteudo));
        m.solto("\n");
    }
    let notas = definicoes_das_notas(doc);
    if !notas.is_empty() {
        m.solto("\n");
        m.solto(&notas);
    }
    let fontes = fontes_em_comentario(doc);
    if !fontes.is_empty() {
        m.solto("\n");
        m.solto(&fontes);
    }
    m
}

/// O Markdown de um bloco sozinho (a "impressão digital" que o modo dividido compara).
pub fn bloco_markdown(c: &Conteudo) -> String {
    match c {
        Conteudo::Titulo { nivel, texto } => format!("{} {}", "#".repeat(usize::from((*nivel).clamp(1, 6))), serializar(texto)),
        Conteudo::Paragrafo { texto } => serializar(texto),
        Conteudo::Item { ordenado, nivel, texto } => format!("{}{} {}", "  ".repeat(usize::from(*nivel)), if *ordenado { "1." } else { "-" }, serializar(texto)),
        Conteudo::Citacao { texto } => format!("> {}", serializar(texto)),
        Conteudo::Codigo { linguagem, codigo } => format!("```{linguagem}\n{codigo}\n```"),
        Conteudo::Tabela { cabecalho, linhas } => tabela_para_fonte(*cabecalho, linhas),
        Conteudo::Imagem { origem, legenda, .. } => format!("![{}]({origem})", serializar(legenda)),
        // Ambiente inteiro (`\begin{align}…`): sai cru, como o pandoc/MathJax esperam; o resto, entre `$$`.
        Conteudo::Equacao { latex } if ambiente_de_exibicao(latex) => latex.clone(),
        Conteudo::Equacao { latex } => format!("$$\n{latex}\n$$"),
        Conteudo::Sumario { lista: 1, .. } => "[LOF]".to_string(),
        Conteudo::Sumario { lista: 2, .. } => "[LOT]".to_string(),
        Conteudo::Sumario { .. } => "[TOC]".to_string(),
        Conteudo::Regua => "---".to_string(),
        Conteudo::QuebraDePagina | Conteudo::QuebraDeColuna => "<div style=\"page-break-after: always\"></div>".to_string(),
        Conteudo::QuebraDeSecao { inicio: InicioDeSecao::Continua } => String::new(),
        Conteudo::QuebraDeSecao { .. } => "<div style=\"page-break-after: always\"></div>".to_string(),
        Conteudo::Bibliografia { todas: false } => "[REFERENCIAS]".to_string(),
        Conteudo::Bibliografia { todas: true } => "[REFERENCIAS: TODAS]".to_string(),
        Conteudo::IndiceRemissivo => "[INDICE]".to_string(),
    }
}

/// O começo do comentário que guarda as fontes no Markdown (o leitor procura por ele).
pub const MARCA_DAS_FONTES: &str = "<!-- FeatherDocs: fontes";

/// As fontes e o estilo num comentário HTML no fim do Markdown (em BibTeX), para o arquivo voltar
/// com as citações inteiras; quem lê com outro programa não vê nada.
fn fontes_em_comentario(doc: &Documento) -> String {
    let b = &doc.bibliografia;
    if b.fontes.is_empty() {
        return String::new();
    }
    format!("{MARCA_DAS_FONTES} estilo={}\n{}-->\n", nome_do_estilo(b.estilo), crate::bib::gravar(&b.fontes).replace("--", "\\-\\-"))
}

/// O nome do estilo de citação (o mesmo que o motor aceita).
pub fn nome_do_estilo(e: EstiloDeCitacao) -> &'static str {
    match e {
        EstiloDeCitacao::Abnt => "abnt",
        EstiloDeCitacao::Apa => "apa",
        EstiloDeCitacao::Vancouver => "vancouver",
        EstiloDeCitacao::Ieee => "ieee",
        EstiloDeCitacao::Chicago => "chicago",
    }
}

fn escapar_tex(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '\\' => s.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                s.push('\\');
                s.push(c);
            }
            '~' => s.push_str("\\textasciitilde{}"),
            '^' => s.push_str("\\textasciicircum{}"),
            featherdocs_core::modelo::QUEBRA_DE_LINHA => s.push_str("\\\\\n"),
            _ => s.push(c),
        }
    }
    s
}

/// Um objeto preso ao texto em LaTeX: `\footnote`, `\endnote`, `\cite`, `\index`, ou o campo.
fn objeto_tex(t: &Trecho, m: &Marca) -> String {
    match m {
        Marca::Nota { fim: false, texto } => format!("\\footnote{{{}}}", texto_tex(texto)),
        Marca::Nota { fim: true, texto } => format!("\\endnote{{{}}}", texto_tex(texto)),
        Marca::Citacao { chaves, pagina } if pagina.is_empty() => format!("\\cite{{{}}}", chaves.join(",")),
        Marca::Citacao { chaves, pagina } => format!("\\cite[p.~{}]{{{}}}", escapar_tex(pagina), chaves.join(",")),
        Marca::Indice { termo, sub } if sub.is_empty() => format!("\\index{{{}}}", escapar_tex(termo)),
        Marca::Indice { termo, sub } => format!("\\index{{{}!{}}}", escapar_tex(termo), escapar_tex(sub)),
        // shortcut: a imagem no meio da linha (ícone, logo) não volta para o `.tex`.
        Marca::Comentario { .. } | Marca::Imagem { .. } | Marca::Controle { .. } => String::new(),
        // O comentário vai até o fim da linha: a quebra depois dele não corta o texto que segue.
        Marca::ObjetoWord { resumo, .. } => format!("%\n% {resumo} do Word: não exportado\n"),
        Marca::Campo { campo } => match campo.as_str() {
            "pagina" => "\\thepage{}".to_string(),
            "paginas" => "\\pageref{LastPage}".to_string(),
            "data" => "\\today{}".to_string(),
            _ => escapar_tex(&t.rotulo),
        },
    }
}

fn trecho_tex(t: &Trecho) -> String {
    if let Some(m) = t.marca.as_deref() {
        return objeto_tex(t, m);
    }
    let mut s = if t.estilo & MATEMATICA != 0 {
        format!("${}$", t.texto)
    } else if t.estilo & CODIGO != 0 {
        format!("\\texttt{{{}}}", escapar_tex(&t.texto))
    } else {
        escapar_tex(&t.texto)
    };
    // O marca-texto vai por dentro (o `\hl` do `soul` não aceita comandos no argumento).
    if t.fmt.realce != 0 {
        s = format!("{{\\sethlcolor{{{}}}\\hl{{{s}}}}}", nome_do_realce(t.fmt.realce));
    }
    for (bit, cmd) in [(NEGRITO, "textbf"), (ITALICO, "emph"), (SUBLINHADO, "underline"), (TACHADO, "sout")] {
        if t.estilo & bit != 0 {
            s = format!("\\{cmd}{{{s}}}");
        }
    }
    if t.fmt.cor != 0 {
        let hex = format!("{:06X}", (t.fmt.cor >> 8) & 0xFFFFFF);
        s = format!("\\textcolor[HTML]{{{hex}}}{{{s}}}");
    }
    match t.fmt.posicao {
        1 => s = format!("\\textsuperscript{{{s}}}"),
        -1 => s = format!("\\textsubscript{{{s}}}"),
        _ => {}
    }
    // O corpo que não é o do texto (11 pt na classe que o FeatherDocs escreve).
    if t.fmt.tamanho > 0.0 && (t.fmt.tamanho - CORPO).abs() > 0.01 {
        s = format!("{{\\fontsize{{{}}}{{{}}}\\selectfont {s}}}", pt(t.fmt.tamanho), pt(t.fmt.tamanho * 1.2));
    }
    match &t.link {
        // O link para um bloco do documento (o `\ref`) leva ao `\label` que o bloco ganha.
        Some(u) if u.starts_with("#bloco:") => format!("\\hyperref[{}]{{{s}}}", rotulo_do_bloco(&u[7..])),
        Some(u) => format!("\\href{{{u}}}{{{s}}}"),
        None => s,
    }
}

/// O `\label` do bloco `id` (o destino dos links internos).
fn rotulo_do_bloco(id: &str) -> String {
    format!("bloco:{id}")
}

/// Os blocos que algum link do documento aponta.
fn destinos_dos_links(doc: &Documento) -> std::collections::HashSet<u64> {
    doc.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().filter_map(|t| t.link.as_deref()?.strip_prefix("#bloco:")?.parse().ok()).collect()
}

/// O `\label` dentro do bloco quando ele é um ambiente (`figure`, `equation`), senão logo depois.
fn com_rotulo(s: String, id: u64) -> String {
    let rotulo = format!("\\label{{{}}}", rotulo_do_bloco(&id.to_string()));
    if let Some(p) = s.rfind("\\end{").filter(|p| s[*p..].matches('\n').count() == 0 && (s.starts_with("\\begin{figure") || s.starts_with("\\begin{equation") || s.starts_with("\\begin{align") || s.starts_with("\\begin{gather"))) {
        return format!("{}{rotulo}\n{}", &s[..p], &s[p..]);
    }
    format!("{s}{rotulo}")
}

/// O corpo do texto na classe que o FeatherDocs escreve (`\documentclass[11pt]`).
const CORPO: f32 = 11.0;

/// Uma medida em pontos com até duas casas ("12", "13.2").
fn pt(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A cor das linhas alternadas da tabela pelo nome que o preâmbulo define (`linhaF2F4F7`).
fn nome_da_cor_de_linha(cor: u32) -> String {
    format!("linha{:06X}", (cor >> 8) & 0xFFFFFF)
}

/// A cor do marca-texto pelo nome que o preâmbulo define (`realceFFFF00`).
fn nome_do_realce(cor: u32) -> String {
    format!("realce{:06X}", (cor >> 8) & 0xFFFFFF)
}

fn texto_tex(ts: &[Trecho]) -> String {
    ts.iter().map(trecho_tex).collect()
}

/// Um fio de `Bloco::latex` (`T`, `M`, `C2-3`…) como comando do LaTeX.
fn fio_tex(f: &str) -> String {
    match f.split_at(1) {
        ("T", _) => "\\toprule".into(),
        ("M", _) => "\\midrule".into(),
        ("B", _) => "\\bottomrule".into(),
        ("H", _) => "\\hline".into(),
        ("C", faixa) => format!("\\cline{{{faixa}}}"),
        ("c", faixa) => format!("\\cmidrule{{{faixa}}}"),
        _ => String::new(),
    }
}

/// As colunas e os fios de cada divisa: os do código LaTeX de onde a tabela veio, ou `l` e `\hline` em tudo.
fn tabela_tex(linhas: &[Vec<Vec<Trecho>>], mesclagens: &[featherdocs_core::modelo::MesclagemTabela], regua: &str) -> String {
    let cols = linhas.iter().map(Vec::len).max().unwrap_or(1);
    let (spec, fios) = match regua.split_once('\n') {
        Some((spec, fios)) => (spec.to_string(), fios.split(';').map(|d| d.split(',').filter(|f| !f.is_empty()).map(fio_tex).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>()),
        None => ("l".repeat(cols), vec!["\\hline".to_string(); linhas.len() + 1]),
    };
    let divisa = |k: usize| fios.get(k).filter(|f| !f.is_empty()).map_or(String::new(), |f| format!(" {f}"));
    let mut s = format!("\\begin{{tabular}}{{{spec}}}\n{}\n", divisa(0).trim_start());
    for (i, l) in linhas.iter().enumerate() {
        let mut cels: Vec<String> = Vec::new();
        let mut c = 0;
        while c < l.len() {
            match mesclagens.iter().find(|m| m.linha == i && m.coluna == c && m.colunas > 1) {
                Some(m) => {
                    // Com `|` nas colunas, a célula mesclada leva o da direita.
                    let barra = if spec.contains('|') && (m.coluna + m.colunas < cols || spec.trim_end().ends_with('|')) { "|" } else { "" };
                    let alin = if regua.is_empty() { "l" } else { "c" };
                    cels.push(format!("\\multicolumn{{{}}}{{{alin}{barra}}}{{{}}}", m.colunas, texto_tex(&l[c])));
                    c += m.colunas;
                }
                None => {
                    cels.push(texto_tex(&l[c]));
                    c += 1;
                }
            }
        }
        s.push_str(&format!("{} \\\\{}\n", cels.join(" & "), divisa(i + 1)));
    }
    s.push_str("\\end{tabular}");
    s
}

/// As linguagens que o `listings` conhece com o mesmo nome (as outras saem em `verbatim`).
const LINGUAGENS_DO_LISTINGS: &[&str] = &["Python", "C", "C++", "Java", "bash", "SQL", "HTML", "XML", "TeX", "Matlab", "R", "Ruby", "Perl", "PHP", "Haskell", "Lisp", "Fortran", "Pascal", "Go", "Octave", "make"];

fn linguagem_do_listings(l: &str) -> Option<&'static str> {
    let l = match l.to_ascii_lowercase().as_str() {
        "sh" | "shell" => "bash".to_string(),
        "cpp" => "c++".to_string(),
        "latex" => "tex".to_string(),
        x => x.to_string(),
    };
    LINGUAGENS_DO_LISTINGS.iter().copied().find(|n| n.to_ascii_lowercase() == l)
}

/// As divisões do LaTeX por nível de título; com capítulos, o nível 1 é `\\chapter`.
const DIVISOES: [&str; 7] = ["chapter", "section", "subsection", "subsubsection", "paragraph", "subparagraph", "subparagraph"];

/// O LaTeX de um bloco sozinho (a "impressão digital" que o modo dividido compara).
pub fn bloco_tex(b: &Bloco, numeracao: Numeracao) -> String {
    let c = &b.conteudo;
    let mut s = match c {
        Conteudo::Titulo { nivel, texto } => {
            let desloc = usize::from(numeracao != Numeracao::Capitulo);
            let cmd = DIVISOES[usize::from((*nivel).clamp(1, 6)) - 1 + desloc];
            // Sem numeração (no documento ou só neste título), a divisão leva estrela.
            let estrela = if numeracao == Numeracao::Nenhuma || b.atrib.sem_numero { "*" } else { "" };
            let apendice = if b.atrib.apendice { "\\appendix\n" } else { "" };
            format!("{apendice}\\{cmd}{estrela}{{{}}}", texto_tex(texto))
        }
        // A legenda numerada solta (a da ABNT, em cima da imagem; a da tabela).
        Conteudo::Paragrafo { texto } if matches!(b.atrib.legenda, 1 | 2) => {
            format!("\\captionof{{{}}}{{{}}}", if b.atrib.legenda == 1 { "figure" } else { "table" }, texto_tex(texto))
        }
        Conteudo::Paragrafo { texto } => texto_tex(texto),
        Conteudo::Item { texto, .. } => format!("\\item {}", texto_tex(texto)),
        Conteudo::Citacao { texto } => format!("\\begin{{quote}}\n{}\n\\end{{quote}}", texto_tex(texto)),
        Conteudo::Codigo { codigo, linguagem } => match linguagem_do_listings(linguagem) {
            Some(l) => format!("\\begin{{lstlisting}}[language={l}]\n{codigo}\n\\end{{lstlisting}}"),
            None => format!("\\begin{{verbatim}}\n{codigo}\n\\end{{verbatim}}"),
        },
        // A entrelinha da tabela é o `\arraystretch` (o grupo limita a ela).
        Conteudo::Tabela { linhas, .. } => {
            let mut t = tabela_tex(linhas, &b.mesclagens, &b.latex);
            if b.atrib.alinhamento == Alinhamento::Centro {
                t = format!("{{\\centering\n{t}\\par}}");
            }
            // O fundo da tabela pinta uma linha sim, outra não (o `\rowcolors` do `xcolor`).
            if b.atrib.fundo != 0 {
                let cor = nome_da_cor_de_linha(b.atrib.fundo);
                let (impar, par) = if b.atrib.marcado { ("white", cor.as_str()) } else { (cor.as_str(), "white") };
                t = format!("\\rowcolors{{2}}{{{impar}}}{{{par}}}\n{t}");
            }
            // A entrelinha da tabela é o `\arraystretch` (o grupo limita a ela).
            if b.atrib.entrelinha > 0.0 || b.atrib.fundo != 0 {
                let estica = if b.atrib.entrelinha > 0.0 { format!("\\renewcommand{{\\arraystretch}}{{{}}}\n", pt(b.atrib.entrelinha / 1.15)) } else { String::new() };
                t = format!("{{{estica}{t}\n}}");
            }
            t
        }
        Conteudo::Imagem { origem, legenda, largura, .. } => {
            let nome = nome_da_imagem(b.id, origem).unwrap_or_else(|| origem.clone());
            let largura = if *largura > 0.0 { format!("{}pt", pt(*largura)) } else { "0.8\\textwidth".to_string() };
            // Sem legenda, sem `\caption` (senão o LaTeX numera uma figura vazia).
            // A legenda de subfigura ("(a) …") não ganha número próprio de figura.
            let subfigura = legenda.first().is_some_and(|t| t.texto.len() >= 4 && t.texto.starts_with('(') && t.texto[1..].starts_with(|c: char| c.is_ascii_lowercase()) && t.texto[2..].starts_with(") "));
            let legenda = match (legenda.is_empty(), subfigura) {
                (true, _) => String::new(),
                (false, true) => format!("\\caption*{{{}}}\n", texto_tex(legenda)),
                (false, false) => format!("\\caption{{{}}}\n", texto_tex(legenda)),
            };
            let desenho = match crate::latex::fonte_tikz(origem) {
                // As `\definecolor` dele vão para o preâmbulo.
                Some(t) => t.lines().filter(|l| !l.starts_with("\\definecolor")).collect::<Vec<_>>().join("\n"),
                // O pdfLaTeX não lê SVG: o pacote `svg` o converte (pelo Inkscape, como no Overleaf).
                None if nome.ends_with(".svg") => format!("\\includesvg[width={largura}]{{{}}}", nome.trim_end_matches(".svg")),
                None => format!("\\includegraphics[width={largura}]{{{nome}}}"),
            };
            // Só `[h]` o LaTeX troca por `[ht]` com aviso.
            format!("\\begin{{figure}}[ht]\n\\centering\n{desenho}\n{legenda}\\end{{figure}}")
        }
        Conteudo::Equacao { latex } if ambiente_de_exibicao(latex) => latex.clone(),
        Conteudo::Equacao { latex } => format!("\\[\n{latex}\n\\]"),
        Conteudo::Sumario { lista: 1, .. } => "\\listoffigures".to_string(),
        Conteudo::Sumario { lista: 2, .. } => "\\listoftables".to_string(),
        Conteudo::Sumario { .. } => "\\tableofcontents".to_string(),
        Conteudo::Regua => "\\hrule".to_string(),
        Conteudo::QuebraDePagina => "\\newpage".to_string(),
        // shortcut: sem multicol a quebra de coluna vira página, e a de página par sai como \newpage; refinar com twoside.
        Conteudo::QuebraDeColuna | Conteudo::QuebraDeSecao { inicio: InicioDeSecao::ProximaPagina | InicioDeSecao::PaginaPar } => "\\newpage".to_string(),
        Conteudo::QuebraDeSecao { inicio: InicioDeSecao::PaginaImpar } => "\\cleardoublepage".to_string(),
        Conteudo::QuebraDeSecao { inicio: InicioDeSecao::Continua } => String::new(),
        Conteudo::Bibliografia { todas } => {
            let filtro = b.latex.strip_prefix("keyword=").map_or(String::new(), |p| format!("[keyword={{{p}}}]"));
            format!("{}\\printbibliography{filtro}", if *todas { "\\nocite{*}\n" } else { "" })
        }
        Conteudo::IndiceRemissivo => "\\printindex".to_string(),
    };
    // Fundo e moldura do parágrafo (o fundo da tabela já são as linhas dela).
    if b.atrib.fundo != 0 && !matches!(c, Conteudo::Tabela { .. }) {
        let hex = |c: u32| format!("{:06X}", (c >> 8) & 0xFFFFFF);
        // O `\parbox` deixa o texto quebrar dentro da caixa (a caixa sozinha é uma linha só).
        // A moldura do `\fcolorbox` soma 2 `\fboxrule` (sem descontar, a caixa passa 0,8pt da margem).
        let moldura = b.atrib.borda != 0 && b.atrib.cor_borda != 0;
        let s_em_caixa = format!("\\parbox{{\\dimexpr\\linewidth-2\\fboxsep{}\\relax}}{{{s}}}", if moldura { "-2\\fboxrule" } else { "" });
        s = if moldura {
            format!("\\noindent\\fcolorbox[HTML]{{{}}}{{{}}}{{{s_em_caixa}}}", hex(b.atrib.cor_borda), hex(b.atrib.fundo))
        } else {
            format!("\\noindent\\colorbox[HTML]{{{}}}{{{s_em_caixa}}}", hex(b.atrib.fundo))
        };
    }
    s
}

fn eh_texto(c: &Conteudo) -> bool {
    matches!(c, Conteudo::Titulo { .. } | Conteudo::Paragrafo { .. } | Conteudo::Citacao { .. })
}

/// O alinhamento da maioria dos parágrafos: é o do documento todo (os outros levam ambiente).
fn alinhamento_padrao(doc: &Documento) -> Alinhamento {
    let esquerda = doc.blocos.iter().filter(|b| eh_texto(&b.conteudo) && b.atrib.alinhamento == Alinhamento::Esquerda).count();
    let justificado = doc.blocos.iter().filter(|b| eh_texto(&b.conteudo) && b.atrib.alinhamento == Alinhamento::Justificado).count();
    if justificado >= esquerda && justificado > 0 { Alinhamento::Justificado } else { Alinhamento::Esquerda }
}

/// O que o parágrafo tem além do texto: alinhamento, recuos, entrelinha e espaço antes e depois.
fn com_paragrafo(b: &Bloco, s: String, padrao: Alinhamento) -> String {
    let a = &b.atrib;
    if !eh_texto(&b.conteudo) {
        return s;
    }
    let mut medidas = String::new();
    for (nome, v) in [("leftskip", a.recuo_esq), ("rightskip", a.recuo_dir), ("parindent", a.primeira)] {
        if v != 0.0 {
            medidas.push_str(&format!("\\setlength{{\\{nome}}}{{{}pt}}", pt(v)));
        }
    }
    // A entrelinha do FeatherDocs é múltiplo de 1,4 × o corpo (1,15); a que veio do LaTeX já é a dele.
    if a.entrelinha > 0.0 && !crate::latex::e_entrelinha_latex(a.entrelinha) {
        medidas.push_str(&format!("\\setlength{{\\baselineskip}}{{{}pt}}", pt(a.entrelinha / 1.15 * 1.4 * CORPO)));
    }
    let mut s = if medidas.is_empty() { s } else { format!("{{{medidas}{s}\\par}}") };
    // A declaração num grupo, não o ambiente `center`: ele põe o `\topsep` em volta, que o parágrafo não tem.
    match a.alinhamento {
        x if x == padrao => {}
        Alinhamento::Centro => s = format!("{{\\centering\n{s}\\par}}"),
        Alinhamento::Direita => s = format!("{{\\raggedleft\n{s}\\par}}"),
        Alinhamento::Esquerda => s = format!("{{\\raggedright\n{s}\\par}}"),
        Alinhamento::Justificado => s = format!("\\begin{{justify}}\n{s}\n\\end{{justify}}"),
    }
    if let Some(v) = a.antes {
        s = format!("\\vspace{{{}pt}}\n{s}", pt(v));
    }
    if let Some(v) = a.depois {
        s = format!("{s}\n\\vspace{{{}pt}}", pt(v));
    }
    if a.encher {
        s = format!("\\vfill\n{s}");
    }
    s
}

/// Um texto do cabeçalho ou do rodapé com os campos trocados pelos comandos do LaTeX.
fn faixa_tex(t: &str) -> String {
    let t = t.replace("{pagina_romano}", "\u{E002}").replace("{pagina}", "\u{E000}").replace("{total}", "\u{E001}").replace("{data}", "\u{E003}");
    escapar_tex(&t).replace('\u{E000}', "\\thepage{}").replace('\u{E001}', "\\pageref{LastPage}").replace('\u{E002}', "\\roman{page}").replace('\u{E003}', "\\today{}")
}

/// O preâmbulo do que vale para a página: recuo, alinhamento, cabeçalho e rodapé, cor do papel,
/// marca d'água, borda da página, números de linha, forma das notas e os marca-textos usados.
fn preambulo_da_pagina(doc: &Documento, padrao: Alinhamento) -> String {
    let x = &doc.extras;
    let hex = |c: u32| format!("{:06X}", (c >> 8) & 0xFFFFFF);
    // O FeatherDocs não recua a primeira linha (o parágrafo que recua diz o recuo dele).
    let mut s = String::from("\\setlength{\\parindent}{0pt}\n");
    if padrao == Alinhamento::Esquerda {
        s.push_str("\\usepackage{ragged2e}\n\\AtBeginDocument{\\RaggedRight}\n");
    }
    let padrao_rodape = featherdocs_core::modelo::Extras::default();
    if x.cabecalho.vazia() && x.rodape.vazia() {
        s.push_str("\\pagestyle{empty}\n");
    } else if x.cabecalho != padrao_rodape.cabecalho || x.rodape != padrao_rodape.rodape {
        s.push_str(&format!(
            "\\usepackage{{fancyhdr}}\n\\pagestyle{{fancy}}\n\\fancyhf{{}}\n\\renewcommand{{\\headrulewidth}}{{{}pt}}\n\\renewcommand{{\\footrulewidth}}{{{}pt}}\n\\setlength{{\\headheight}}{{14pt}}\n",
            x.cabecalho.fio, x.rodape.fio
        ));
        for (cmd, f) in [("fancyhead", &x.cabecalho), ("fancyfoot", &x.rodape)] {
            for (lado, t) in [("L", &f.esquerda), ("C", &f.centro), ("R", &f.direita)] {
                if !t.is_empty() {
                    s.push_str(&format!("\\{cmd}[{lado}]{{{}}}\n", faixa_tex(t)));
                }
            }
        }
    }
    if x.cor_pagina != 0 {
        s.push_str(&format!("\\pagecolor[HTML]{{{}}}\n", hex(x.cor_pagina)));
    }
    if !x.marca.is_empty() {
        s.push_str(&format!("\\usepackage{{draftwatermark}}\n\\SetWatermarkText{{{}}}\n\\SetWatermarkScale{{1}}\n", escapar_tex(&x.marca)));
        if x.marca_cor != 0 {
            s.push_str(&format!("\\SetWatermarkColor[HTML]{{{}}}\n", hex(x.marca_cor)));
        }
    }
    if x.borda_cor != 0 && x.borda_espessura > 0.0 {
        // Quatro réguas no fundo de cada página (a origem do `shipout/background` é o canto de cima).
        let p = &doc.pagina;
        let (d, e) = (x.borda_distancia, x.borda_espessura);
        let (l, a) = (p.largura - 2.0 * d, p.altura - 2.0 * d);
        let base = -(p.altura - d);
        s.push_str(&format!(
            "\\AddToHook{{shipout/background}}{{\\setlength{{\\unitlength}}{{1pt}}\\color[HTML]{{{}}}%\n  \\put({d},{}){{\\rule{{{l}pt}}{{{e}pt}}}}\\put({d},{base}){{\\rule{{{l}pt}}{{{e}pt}}}}%\n  \\put({d},{base}){{\\rule{{{e}pt}}{{{a}pt}}}}\\put({},{base}){{\\rule{{{e}pt}}{{{a}pt}}}}}}\n",
            hex(x.borda_cor), pt(-(d + e)), pt(p.largura - d - e),
            d = pt(d), e = pt(e), l = pt(l), a = pt(a), base = pt(base),
        ));
    }
    if x.numerar_linhas != featherdocs_core::modelo::NumeracaoDeLinhas::Nenhuma {
        s.push_str("\\usepackage{lineno}\n");
    }
    match x.formato_notas {
        1 => s.push_str("\\renewcommand{\\thefootnote}{\\alph{footnote}}\n"),
        2 => s.push_str("\\renewcommand{\\thefootnote}{\\roman{footnote}}\n"),
        3 => s.push_str("\\renewcommand{\\thefootnote}{\\fnsymbol{footnote}}\n"),
        _ => {}
    }
    let mut linhas: Vec<u32> = doc.blocos.iter().filter(|b| matches!(b.conteudo, Conteudo::Tabela { .. }) && b.atrib.fundo != 0).map(|b| b.atrib.fundo).collect();
    linhas.sort_unstable();
    linhas.dedup();
    for c in linhas {
        s.push_str(&format!("\\definecolor{{{}}}{{HTML}}{{{}}}\n", nome_da_cor_de_linha(c), hex(c)));
    }
    let mut realces: Vec<u32> = doc.blocos.iter().flat_map(|b| trechos_de(&b.conteudo)).map(|t| t.fmt.realce).filter(|&c| c != 0).collect();
    realces.sort_unstable();
    realces.dedup();
    if !realces.is_empty() {
        s.push_str("\\usepackage{soul}\n");
        for c in realces {
            s.push_str(&format!("\\definecolor{{{}}}{{HTML}}{{{}}}\n", nome_do_realce(c), hex(c)));
        }
    }
    s
}

/// O começo do corpo: capa sem cabeçalho, números de linha e o texto no meio ou embaixo da página.
fn inicio_do_corpo(doc: &Documento) -> String {
    use featherdocs_core::modelo::{NumeracaoDeLinhas, Vertical};
    let x = &doc.extras;
    let mut s = String::new();
    if x.primeira_diferente {
        s.push_str("\\thispagestyle{empty}\n");
    }
    match x.numerar_linhas {
        NumeracaoDeLinhas::Continua => s.push_str("\\linenumbers\n"),
        NumeracaoDeLinhas::PorPagina => s.push_str("\\pagewiselinenumbers\n"),
        NumeracaoDeLinhas::Nenhuma => {}
    }
    if x.vertical != Vertical::Topo {
        s.push_str("\\vspace*{\\fill}\n");
    }
    s
}

fn trechos_de(c: &Conteudo) -> Vec<&Trecho> {
    match c {
        Conteudo::Titulo { texto, .. } | Conteudo::Paragrafo { texto } | Conteudo::Item { texto, .. } | Conteudo::Citacao { texto } => texto.iter().collect(),
        Conteudo::Tabela { linhas, .. } => linhas.iter().flatten().flatten().collect(),
        Conteudo::Imagem { legenda, .. } => legenda.iter().collect(),
        _ => Vec::new(),
    }
}

/// O nome de arquivo da imagem embutida (`data:` URL) no LaTeX: `figura-<id>.png`. O `.tex`
/// sozinho só cita o nome; o `.zip` do projeto ([`latex_zip`]) leva o arquivo junto.
pub fn nome_da_imagem(id: u64, origem: &str) -> Option<String> {
    let tipo = origem.strip_prefix("data:image/")?.split([';', ',']).next()?;
    let ext = match tipo {
        "jpeg" => "jpg",
        "svg+xml" => "svg",
        t => t,
    };
    Some(format!("figura-{id}.{ext}"))
}

/// O projeto LaTeX num `.zip` (como o Overleaf abre): `main.tex` e as imagens embutidas.
pub fn latex_zip(doc: &Documento) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opcoes = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let erro = |e: &dyn std::fmt::Display| format!("Não consegui montar o ZIP: {e}");
    z.start_file("main.tex", opcoes).map_err(|e| erro(&e))?;
    z.write_all(latex(doc).as_bytes()).map_err(|e| erro(&e))?;
    for b in &doc.blocos {
        let Conteudo::Imagem { origem, .. } = &b.conteudo else { continue };
        let (Some(nome), Some((_, bytes))) = (nome_da_imagem(b.id, origem), crate::base64::de_data_url(origem)) else { continue };
        z.start_file(nome, opcoes).map_err(|e| erro(&e))?;
        z.write_all(&bytes).map_err(|e| erro(&e))?;
    }
    Ok(z.finish().map_err(|e| erro(&e))?.into_inner())
}

/// O documento como LaTeX (classe `article`, com `babel` em português).
pub fn latex(doc: &Documento) -> String {
    latex_com_mapa(doc).texto
}

/// O mapa (bloco → linhas) de um código escrito pelo usuário, que não foi gerado pelo FeatherDocs:
/// cada bloco é procurado pelas palavras dele, em ordem, sem diferença de caixa (o `\MakeUppercase`
/// e as macros mudam a forma, não as palavras). No LaTeX a busca começa depois de `\begin{document}`
/// e pula comentários. O bloco vai da linha onde foi achado até a do bloco seguinte.
pub fn mapa_de_texto(texto: &str, doc: &Documento, fonte: crate::dividido::Fonte) -> Mapa {
    let linhas: Vec<String> = texto
        .lines()
        .map(|l| if fonte == crate::dividido::Fonte::Latex && l.trim_start().starts_with('%') { String::new() } else { l.to_lowercase() })
        .collect();
    let inicio = if fonte == crate::dividido::Fonte::Latex { linhas.iter().position(|l| l.contains("\\begin{document}")).map_or(0, |p| p + 1) } else { 0 };
    let mut blocos: Vec<LinhasDoBloco> = Vec::new();
    let mut ultima = inicio;
    for b in &doc.blocos {
        let palavras = palavras_do_bloco(&b.conteudo);
        let Some(primeira) = palavras.first() else { continue };
        let tem = |k: usize, p: &str| linhas[k].contains(p) || linhas.get(k + 1).is_some_and(|l| l.contains(p));
        let achado = (ultima..linhas.len())
            .find(|&k| linhas[k].contains(primeira.as_str()) && palavras[1..].iter().all(|p| tem(k, p)))
            .or_else(|| (ultima..linhas.len()).find(|&k| linhas[k].contains(primeira.as_str())));
        if let Some(k) = achado {
            blocos.push(LinhasDoBloco { id: b.id, de: k, ate: k + 1 });
            ultima = k;
        }
    }
    for i in 0..blocos.len().saturating_sub(1) {
        let proximo = blocos[i + 1].de;
        if proximo > blocos[i].de {
            blocos[i].ate = proximo;
        }
    }
    Mapa { texto: texto.to_string(), blocos }
}

/// Até três palavras (em minúsculas) do texto de um bloco.
fn palavras_do_bloco(c: &Conteudo) -> Vec<String> {
    let texto = match c {
        Conteudo::Titulo { texto, .. } | Conteudo::Paragrafo { texto } | Conteudo::Item { texto, .. } | Conteudo::Citacao { texto } => texto_puro(texto),
        Conteudo::Codigo { codigo, .. } => codigo.clone(),
        Conteudo::Tabela { linhas, .. } => linhas.iter().flatten().map(|c| texto_puro(c)).collect::<Vec<_>>().join(" "),
        Conteudo::Equacao { latex } => latex.clone(),
        _ => String::new(),
    };
    let palavras: Vec<&str> = texto.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
    // As de três letras ou mais distinguem melhor; um bloco só de palavras curtas ("Um", "T") usa as que tem.
    let longas: Vec<&str> = palavras.iter().copied().filter(|p| p.chars().count() >= 3).collect();
    let escolhidas = if longas.is_empty() { palavras } else { longas };
    escolhidas.into_iter().take(3).map(str::to_lowercase).collect()
}

/// O papel e as margens em `\\usepackage[…]{geometry}`, em cm (o que o leitor de LaTeX entende de volta).
fn geometria(p: &featherdocs_core::modelo::ConfigPagina) -> String {
    let cm = |pt: f32| format!("{:.2}cm", pt / 72.0 * 2.54);
    let (curto, longo) = (p.largura.min(p.altura), p.largura.max(p.altura));
    let perto = |a: f32, b: f32| (a - b).abs() < 1.0;
    let papel = if perto(curto, 595.28) && perto(longo, 841.89) {
        "a4paper".to_string()
    } else if perto(curto, 612.0) && perto(longo, 792.0) {
        "letterpaper".to_string()
    } else {
        format!("paperwidth={}, paperheight={}", cm(curto), cm(longo))
    };
    let deitado = if p.largura > p.altura { ", landscape" } else { "" };
    format!("\\usepackage[{papel}{deitado}, top={}, bottom={}, left={}, right={}]{{geometry}}\n", cm(p.margem_sup), cm(p.margem_inf), cm(p.margem_esq), cm(p.margem_dir))
}

/// Como `latex`, com as linhas de cada bloco.
pub fn latex_com_mapa(doc: &Documento) -> Mapa {
    let doc = &*sem_sugestoes(doc);
    let classe = if doc.estilos.numeracao == Numeracao::Capitulo { "report" } else { "article" };
    let usa = objetos_usados(doc);
    // As fontes vão dentro do próprio .tex (o `filecontents` grava o .bib ao compilar).
    let mut s = if usa.citacoes { format!("\\begin{{filecontents*}}[overwrite]{{\\jobname.bib}}\n{}\\end{{filecontents*}}\n", crate::bib::gravar(&doc.bibliografia.fontes)) } else { String::new() };
    s.push_str(&format!("\\documentclass[11pt]{{{classe}}}\n"));
    s.push_str("\\usepackage[utf8]{inputenc}\n\\usepackage[T1]{fontenc}\n\\usepackage[brazilian]{babel}\n\\usepackage{amsmath}\n\\usepackage{amssymb}\n\\usepackage{graphicx}\n\\usepackage[hypcap=false]{caption}\n\\usepackage{hyperref}\n\\hypersetup{pdfborder={0 0 0}}\n\\usepackage[normalem]{ulem}\n\\usepackage[dvipsnames, table]{xcolor}\n");
    s.push_str(&geometria(&doc.pagina));
    let padrao = alinhamento_padrao(doc);
    s.push_str(&preambulo_da_pagina(doc, padrao));
    if usa.citacoes {
        // O biblatex com babel pede o csquotes.
        s.push_str("\\usepackage{csquotes}\n");
        s.push_str(pacote_de_citacao(doc.bibliografia.estilo));
        s.push_str("\\addbibresource{\\jobname.bib}\n");
    }
    if usa.notas_de_fim {
        s.push_str("\\usepackage{endnotes}\n");
    }
    if usa.indice {
        s.push_str("\\usepackage{makeidx}\n\\makeindex\n");
    }
    let tem_total = |f: &featherdocs_core::modelo::Faixas| [&f.esquerda, &f.centro, &f.direita].iter().any(|t| t.contains("{total}"));
    if usa.paginas || tem_total(&doc.extras.cabecalho) || tem_total(&doc.extras.rodape) {
        s.push_str("\\usepackage{lastpage}\n");
    }
    let svgs: Vec<&str> = doc.blocos.iter().filter_map(|b| match &b.conteudo {
        Conteudo::Imagem { origem, .. } if origem.starts_with("data:image/svg") => Some(origem.as_str()),
        _ => None,
    }).collect();
    if svgs.iter().any(|o| crate::latex::fonte_tikz(o).is_some()) {
        s.push_str("\\usepackage{tikz}\n\\usetikzlibrary{arrows.meta,positioning,shapes,calc}\n");
        let mut cores: Vec<String> = svgs.iter().filter_map(|o| crate::latex::fonte_tikz(o)).flat_map(|t| t.lines().filter(|l| l.starts_with("\\definecolor")).map(|l| format!("{l}\n")).collect::<Vec<_>>()).collect();
        cores.sort();
        cores.dedup();
        s.push_str(&cores.concat());
    }
    if svgs.iter().any(|o| crate::latex::fonte_tikz(o).is_none()) {
        s.push_str("\\usepackage{svg}\n");
    }
    if doc.blocos.iter().any(|b| b.latex.split_once('\n').is_some_and(|(_, f)| f.contains(['T', 'M', 'B', 'c']))) {
        s.push_str("\\usepackage{booktabs}\n");
    }
    if doc.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::Item { .. }) && rotulo_da_lista(b).is_some()) {
        s.push_str("\\usepackage{enumitem}\n");
    }
    if doc.blocos.iter().any(|b| matches!(&b.conteudo, Conteudo::Codigo { linguagem, .. } if linguagem_do_listings(linguagem).is_some())) {
        // O `listings` do pdfLaTeX não aceita letra acentuada: cada uma vira o acento do TeX.
        s.push_str("\\usepackage{listings}\n\\lstset{literate=");
        for (c, tex) in [("á", "\\'a"), ("à", "\\`a"), ("â", "\\^a"), ("ã", "\\~a"), ("é", "\\'e"), ("ê", "\\^e"), ("í", "\\'i"), ("ó", "\\'o"), ("ô", "\\^o"), ("õ", "\\~o"), ("ú", "\\'u"), ("ü", "\\\"u"), ("ç", "\\c{c}"), ("Á", "\\'A"), ("À", "\\`A"), ("Â", "\\^A"), ("Ã", "\\~A"), ("É", "\\'E"), ("Ê", "\\^E"), ("Í", "\\'I"), ("Ó", "\\'O"), ("Ô", "\\^O"), ("Õ", "\\~O"), ("Ú", "\\'U"), ("Ç", "\\c{C}"), ("º", "\\textordmasculine{}"), ("ª", "\\textordfeminine{}"), ("°", "\\textdegree{}")] {
            s.push_str(&format!("{{{c}}}{{{{{tex}}}}}1 "));
        }
        s.push_str("}\n");
    }
    if !doc.meta.titulo.is_empty() {
        s.push_str(&format!("\\title{{{}}}\n", escapar_tex(&doc.meta.titulo)));
    }
    if !doc.meta.autor.is_empty() {
        s.push_str(&format!("\\author{{{}}}\n", escapar_tex(&doc.meta.autor)));
    }
    // Sem `\maketitle`: o título das propriedades não aparece na página do FeatherDocs (o que aparece
    // é o parágrafo de título que a pessoa escreveu, e ele sai como os outros).
    s.push_str("\\begin{document}\n");
    s.push_str(&inicio_do_corpo(doc));
    let mut m = Mapa { texto: s, blocos: Vec::new() };
    // As listas abertas, de fora para dentro: (ambiente, opções do `enumitem`).
    let mut abertas: Vec<(&str, String)> = Vec::new();
    let destinos = destinos_dos_links(doc);
    let mut trocas = Vec::new();
    for b in &doc.blocos {
        let (ambiente, nivel) = match &b.conteudo {
            Conteudo::Item { ordenado, nivel, .. } => (Some((if *ordenado { "enumerate" } else { "itemize" }, rotulo_da_lista(b).unwrap_or_default())), usize::from(*nivel) + 1),
            _ => (None, 0),
        };
        while abertas.len() > nivel || (abertas.len() == nivel && ambiente.as_ref() != abertas.last()) {
            if let Some((a, _)) = abertas.pop() {
                m.solto(&format!("\\end{{{a}}}\n"));
            }
        }
        while let Some(a) = ambiente.as_ref().filter(|_| abertas.len() < nivel) {
            m.solto(&format!("\\begin{{{}}}{}\n", a.0, a.1));
            abertas.push(a.clone());
        }
        let mut t = com_paragrafo(b, bloco_tex(b, doc.estilos.numeracao), padrao);
        if destinos.contains(&b.id) {
            match t.find("\\label{").and_then(|p| grupo_simples(&t[p + 7..])) {
                Some(r) => trocas.push((rotulo_do_bloco(&b.id.to_string()), r.to_string())),
                None => t = com_rotulo(t, b.id),
            }
        }
        m.bloco(b.id, &t);
        m.solto("\n\n");
    }
    while let Some((a, _)) = abertas.pop() {
        m.solto(&format!("\\end{{{a}}}\n"));
    }
    if usa.notas_de_fim {
        m.solto("\\theendnotes\n");
    }
    if doc.extras.vertical == featherdocs_core::modelo::Vertical::Centro {
        m.solto("\\vspace*{\\fill}\n");
    }
    m.solto("\\end{document}\n");
    for (de, para) in trocas {
        m.texto = m.texto.replace(&format!("\\hyperref[{de}]"), &format!("\\hyperref[{para}]"));
    }
    m
}

/// O texto até a `}` que fecha o `\label{…`.
fn grupo_simples(s: &str) -> Option<&str> {
    s.find('}').map(|f| &s[..f])
}

/// As opções do `enumitem` para o marcador que não é o padrão do nível (`[label=\alph*)]`).
fn rotulo_da_lista(b: &Bloco) -> Option<String> {
    use featherdocs_core::modelo::Lista;
    let r = match b.atrib.lista {
        Lista::Alfabetica => "\\alph*)",
        Lista::Romana => "\\Roman*.",
        Lista::RomanaMinuscula => "\\roman*.",
        Lista::Numerica => "\\arabic*.",
        Lista::Circulo => "$\\circ$",
        Lista::Quadrado => "$\\blacksquare$",
        Lista::Seta => "$\\rightarrow$",
        Lista::Tarefa => "$\\square$",
        Lista::Padrao => return None,
    };
    Some(format!("[label={r}]"))
}

/// O `biblatex` no estilo do documento.
fn pacote_de_citacao(e: EstiloDeCitacao) -> &'static str {
    match e {
        EstiloDeCitacao::Abnt => "\\usepackage[style=abnt]{biblatex}\n",
        EstiloDeCitacao::Apa => "\\usepackage[style=apa]{biblatex}\n",
        EstiloDeCitacao::Vancouver => "\\usepackage[style=vancouver]{biblatex}\n",
        EstiloDeCitacao::Ieee => "\\usepackage[style=ieee]{biblatex}\n",
        EstiloDeCitacao::Chicago => "\\usepackage[authordate]{biblatex-chicago}\n",
    }
}

/// Os pacotes que os objetos do documento pedem.
#[derive(Default)]
struct Usados {
    notas_de_fim: bool,
    indice: bool,
    paginas: bool,
    citacoes: bool,
}

fn objetos_usados(doc: &Documento) -> Usados {
    let mut u = Usados { citacoes: !doc.bibliografia.fontes.is_empty(), ..Usados::default() };
    for b in &doc.blocos {
        match b.conteudo {
            Conteudo::Bibliografia { .. } => u.citacoes = true,
            Conteudo::IndiceRemissivo => u.indice = true,
            _ => {}
        }
    }
    for m in doc.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().filter_map(|t| t.marca.as_deref()) {
        match m {
            Marca::Nota { fim: true, .. } => u.notas_de_fim = true,
            Marca::Indice { .. } => u.indice = true,
            Marca::Campo { campo } if campo == "paginas" => u.paginas = true,
            Marca::Citacao { .. } => u.citacoes = true,
            _ => {}
        }
    }
    u
}

/// O texto puro do documento (para busca e contagem fora do motor).
pub fn texto_simples(doc: &Documento) -> String {
    doc.blocos
        .iter()
        .filter_map(|b| match &b.conteudo {
            Conteudo::Titulo { texto, .. } | Conteudo::Paragrafo { texto } | Conteudo::Item { texto, .. } | Conteudo::Citacao { texto } => Some(texto_puro(texto)),
            Conteudo::Codigo { codigo, .. } => Some(codigo.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
