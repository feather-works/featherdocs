//! LaTeX (o subconjunto de documento) para a árvore de blocos: seções, listas
//! (aninhadas), tabelas `tabular`, figuras, fórmulas (`$…$`, `\[…\]`, `equation`,
//! `align`…), código (`verbatim`, `lstlisting`, `minted`), citações e o texto em
//! linha (`\textbf`, `\emph`, `\texttt`, `\href`, acentos, travessões).
//!
//! Não é um compilador completo de TeX, mas expande macros do usuário (`\newcommand`,
//! `\renewcommand`), reconhece cores (`xcolor`, `\definecolor`, `\color`, `\textcolor`, `\colorbox`),
//! `minipage` e `tabularx`.

use std::collections::HashMap;

use featherdocs_core::inline::texto_puro;
use featherdocs_core::modelo::{
    Alinhamento, Atrib, Bloco, Celula, Conteudo, Documento, EstiloDeCitacao, Faixas, Fmt, Marca, Vertical, Numeracao, Trecho, CODIGO, ITALICO, MATEMATICA, NEGRITO, SERIFA, SUBLINHADO, TACHADO,
};

#[derive(Clone, Debug, Default)]
struct MacroDef {
    args: usize,
    corpo: String,
    /// O padrão de cada argumento opcional (`[…]` no uso); vazio = todos obrigatórios.
    padroes: Vec<Option<String>>,
    /// Num ambiente (`\newenvironment`), o código do `\end{…}`.
    fim: Option<String>,
}

/// A chave de um ambiente no mapa das macros (um comando nunca começa com `@`).
fn chave_de_ambiente(nome: &str) -> String {
    format!("@env:{nome}")
}

/// `\newcommand`/`\newenvironment` (e as variantes `re…`, `provide…`, `…x` do `xargs`): a chave no mapa,
/// a definição e quanto foi lido. As definições dentro do corpo (com `##1`) vêm em `dentro`.
fn definicao_latex(r: &str) -> Option<(String, MacroDef, usize)> {
    if !r.starts_with('\\') {
        return None;
    }
    let (cmd, n) = comando(r);
    let ambiente = match cmd.trim_end_matches('x') {
        "newcommand" | "renewcommand" | "providecommand" | "DeclareRobustCommand" => false,
        "newenvironment" | "renewenvironment" => true,
        _ => return None,
    };
    let x = cmd.ends_with('x');
    let mut p = n;
    let nome = if let Some((g, k)) = grupo(&r[p..]) {
        p += k;
        g.trim().trim_start_matches('\\').to_string()
    } else {
        let depois = r[p..].trim_start();
        if !depois.starts_with('\\') {
            return None;
        }
        let (c, k) = comando(depois);
        p += r[p..].len() - depois.len() + k;
        c.to_string()
    };
    // Parâmetros do LaTeX (`\arraystretch`, `\bibname`…) não são texto: o leitor os trata onde
    // são mudados; expandidos, o `\renewcommand{\arraystretch}{1.5}` viraria `{1.5}{1.5}`.
    if !ambiente && PARAMETROS.contains(&nome.as_str()) {
        return None;
    }
    let mut args = 0;
    if let Some((o, k)) = opcional(&r[p..]) {
        args = o.trim().parse::<usize>().unwrap_or(0);
        p += k;
    }
    let mut padroes = vec![None; args];
    if let Some((o, k)) = opcional(&r[p..]) {
        p += k;
        if x {
            for (i, v) in opcoes_chave_valor(o) {
                if let Some(slot) = i.parse::<usize>().ok().and_then(|i| padroes.get_mut(i.wrapping_sub(1))) {
                    *slot = Some(v.strip_prefix('{').and_then(|v| v.strip_suffix('}')).unwrap_or(&v).to_string());
                }
            }
        } else if let Some(slot) = padroes.first_mut() {
            *slot = Some(o.to_string());
        }
    }
    if padroes.iter().all(Option::is_none) {
        padroes.clear();
    }
    let (corpo, k) = grupo(&r[p..])?;
    p += k;
    let fim = if ambiente {
        let (f, k) = grupo(&r[p..])?;
        p += k;
        Some(f.to_string())
    } else {
        None
    };
    let chave = if ambiente { chave_de_ambiente(&nome) } else { nome };
    Some((chave, MacroDef { args, corpo: corpo.to_string(), padroes, fim }, p))
}

/// O corpo com `#1`…`#9` trocados pelos argumentos; o `##` (de uma definição de dentro) vira `#`.
fn substituir_argumentos(corpo: &str, args: &[String]) -> String {
    let mut saida = String::with_capacity(corpo.len());
    let mut cs = corpo.chars().peekable();
    while let Some(c) = cs.next() {
        if c != '#' {
            saida.push(c);
            continue;
        }
        match cs.peek().copied() {
            Some('#') => {
                cs.next();
                saida.push('#');
            }
            Some(d) if d.is_ascii_digit() => {
                cs.next();
                let i = d.to_digit(10).unwrap_or(0) as usize;
                // `\centering#1` com "Modelo": no TeX são dois tokens.
                colar(&mut saida, args.get(i.wrapping_sub(1)).map_or("", String::as_str));
            }
            _ => saida.push('#'),
        }
    }
    saida
}

/// Os argumentos de um uso: o opcional (`[…]`, ou o padrão) e os obrigatórios (`{…}`). `None` se faltou um.
fn argumentos_do_uso(m: &MacroDef, texto: &str) -> Option<(Vec<String>, usize)> {
    let mut pos = 0;
    let mut args = Vec::with_capacity(m.args);
    for i in 0..m.args {
        match m.padroes.get(i).cloned().flatten() {
            Some(padrao) => match opcional(&texto[pos..]) {
                Some((o, k)) => {
                    args.push(o.to_string());
                    pos += k;
                }
                None => args.push(padrao),
            },
            None => {
                let (g, k) = grupo(&texto[pos..])?;
                args.push(g.to_string());
                pos += k;
            }
        }
    }
    Some((args, pos))
}

mod avisos;
mod exam;
mod icones;
mod linha;
mod pacotes;
pub mod projeto;
mod tabelas;
mod tikz;
mod unidades;

pub use avisos::{Aviso, Leitura, TipoDeAviso};
pub use pacotes::fonte_tikz;

/// Lê um arquivo `.tex`.
pub fn ler(fonte: &str) -> Documento {
    ler_com_avisos(fonte).documento
}

/// Lê um `.tex` e diz o que não entendeu (o aviso de compatibilidade). Sem pasta, o `\input` não
/// é resolvido (e entra no aviso).
pub fn ler_com_avisos(fonte: &str) -> Leitura {
    ler_na_pasta(fonte, None)
}

/// Como `ler_com_avisos`, com os arquivos de `pasta` (a do `.tex`): `\input`, `.bib` e imagens.
pub fn ler_na_pasta(fonte: &str, pasta: Option<&std::path::Path>) -> Leitura {
    match pasta {
        Some(p) => ler_projeto(fonte, &projeto::da_pasta(p)),
        None => ler_sem_projeto(fonte),
    }
}

/// Lê o `.tex` principal de um projeto, pegando os outros arquivos em `arquivos`.
pub fn ler_projeto(fonte: &str, arquivos: projeto::Arquivos) -> Leitura {
    let coletor = avisos::Coletor::default();
    let fonte_inteira = projeto::classes_e_pacotes(&projeto::incluir(fonte, arquivos, &coletor, 0), arquivos);
    let mut documento = ler_texto(&fonte_inteira, &coletor, Some(arquivos), &[]);
    projeto::embutir_imagens(&mut documento, arquivos, &coletor);
    Leitura { avisos: coletor.consolidar(&fonte_inteira), documento }
}

/// O texto sozinho (colado, ou o do painel de código): sem pasta, as imagens e o `.bib` de fora
/// ficam pelo nome, sem aviso; o `\input` sem arquivo continua no aviso.
fn ler_sem_projeto(fonte: &str) -> Leitura {
    let coletor = avisos::Coletor::default();
    let fonte_inteira = projeto::incluir(fonte, &|_| None, &coletor, 0);
    let mut documento = ler_texto(&fonte_inteira, &coletor, None, &[]);
    projeto::sem_escala(&mut documento);
    Leitura { avisos: coletor.consolidar(&fonte_inteira), documento }
}

/// Relê o código do painel: sem os arquivos do projeto à mão, o `.bib` de fora (`\bibliography`)
/// vem das `fontes` que o documento já tem, e o `\citeauthor` escreve o mesmo autor de antes.
pub fn ler_com_fontes(fonte: &str, fontes: &[featherdocs_core::modelo::Fonte]) -> Leitura {
    let coletor = avisos::Coletor::default();
    let inteira = projeto::incluir(fonte, &|_| None, &coletor, 0);
    let mut documento = ler_texto(&inteira, &coletor, None, fontes);
    projeto::sem_escala(&mut documento);
    Leitura { avisos: coletor.consolidar(&inteira), documento }
}

fn ler_texto(fonte: &str, coletor: &avisos::Coletor, arquivos: Option<projeto::Arquivos>, reserva: &[featherdocs_core::modelo::Fonte]) -> Documento {
    let limpo = sem_comentarios(fonte);
    let (preambulo, corpo) = separar(&limpo);
    pacotes::avisar_preambulo(preambulo, coletor);
    // Do `\captionsetup` só o `font=bf` e o `labelsep` são reproduzidos (o resto vai para o aviso).
    let opcoes_legenda: Vec<(String, String)> = preambulo.find("\\captionsetup").and_then(|p| grupo(&preambulo[p + 13..])).map(|(g, _)| opcoes_chave_valor(g)).unwrap_or_default();
    let legenda_negrito = opcoes_legenda.iter().any(|(k, v)| k == "font" && v.split(',').any(|x| x.trim() == "bf"));
    if opcoes_legenda.iter().any(|(k, v)| match k.as_str() {
        "font" => v.split(',').any(|x| x.trim() != "bf"),
        "labelsep" => !matches!(v.trim(), "colon" | "period" | "endash"),
        _ => true,
    }) {
        coletor.anotar(TipoDeAviso::SemEfeito, "\\captionsetup");
    }
    let (cores, macros) = extrair_definicoes(&limpo);
    let mut doc = Documento::vazio();
    doc.extras.composicao_tex = true;
    doc.meta.titulo = argumento_de(preambulo, "\\title", &cores).unwrap_or_default();
    doc.meta.autor = argumento_de(preambulo, "\\author", &cores).unwrap_or_default();
    // O preâmbulo passa pela expansão antes do corpo, na ordem: o que ele faz (`\participante{…}` enchendo
    // uma lista) vale no texto. Só o corpo fica.
    const INICIO_DO_CORPO: &str = "\u{E000}corpo\u{E000}";
    let junto = expandir_macros(format!("{preambulo}{INICIO_DO_CORPO}{corpo}"), &macros);
    let corpo_expandido = junto.split_once(INICIO_DO_CORPO).map_or(junto.clone(), |(_, c)| c.to_string());
    let corpo_expandido = if avisos::classe_do_preambulo(preambulo).as_deref() == Some("exam") { exam::reescrever(&corpo_expandido, preambulo, Classe::do_preambulo(preambulo)) } else { corpo_expandido };
    // As fontes antes do texto: o `\citeauthor`/`\citeyear` escrevem o autor e o ano delas.
    ler_fontes(&mut doc, &limpo, preambulo);
    if let Some(a) = arquivos {
        projeto::fontes_de_fora(&mut doc, &limpo, a, coletor);
    }
    if doc.bibliografia.fontes.is_empty() {
        doc.bibliografia.fontes = reserva.to_vec();
    }
    // Na ABNT (`abntex2cite`) o `\citeauthor` sai em maiúsculas: "OLIVEIRA (2020)".
    let maiusculas = doc.bibliografia.estilo == EstiloDeCitacao::Abnt;
    let fontes = doc
        .bibliografia
        .fontes
        .iter()
        .map(|f| {
            let a = autores_no_texto(&f.autores, if maiusculas { "; " } else { " e " });
            (f.chave.clone(), (if maiusculas { a.to_uppercase().replace(" ET AL.", " et al.") } else { a }, f.ano.clone()))
        })
        .collect();
    let classe = Classe::do_preambulo(preambulo);
    configurar_pagina(&mut doc, preambulo);
    cabecalho_e_rodape(&mut doc, preambulo, corpo, &cores, &macros, classe);
    // A capa (`titlepage`) sai sem cabeçalho e sem rodapé.
    if corpo_expandido.trim_start().starts_with("\\begin{titlepage}") {
        let x = &mut doc.extras;
        (x.cabecalho_primeira, x.rodape_primeira, x.primeira_diferente) = (Faixas::default(), Faixas::default(), true);
        // E não conta: o `titlepage` volta o contador, a página seguinte é a 1.
        x.pagina_inicial = 0;
    }
    extras_da_pagina(&mut doc, preambulo, corpo, &cores);
    let (itemsep, cor_marcador) = listas_do_preambulo(preambulo, &cores, classe);
    let teoremas = pacotes::teoremas_do_preambulo(preambulo);
    let (rotulos, numeros_teoremas) = numeros_dos_rotulos(&corpo_expandido, &teoremas);
    let separador_legenda = match opcoes_legenda.iter().find(|(k, _)| k == "labelsep").map(|(_, v)| v.trim()) {
        Some("colon") => ":",
        Some("period") => ".",
        Some(_) => "",
        None if matches!(avisos::classe_do_preambulo(preambulo).as_deref(), Some("article" | "report" | "book") | None) => ":",
        None => "",
    };
    let ctx = Ctx {
        separador_legenda,
        capitulo: corpo_expandido.contains("\\chapter"),
        titulo: doc.meta.titulo.clone(),
        autor: doc.meta.autor.clone(),
        classe,
        // O `\parskip` do LaTeX é 0 (só estica para encher a página); o pacote `parskip` põe meia linha.
        parskip: comprimento_definido(preambulo, "\\parskip", classe).or(Some(if preambulo.contains("{parskip}") { 0.6 * classe.tamanhos[4] } else { 0.0 })),
        itemsep,
        cor_marcador,
        cores,
        // O LaTeX justifica o texto por padrão; `\raggedright` no preâmbulo (ou `ragged2e`) tira.
        fontes: std::rc::Rc::new(fontes),
        rotulos: std::rc::Rc::new(rotulos),
        teoremas: std::rc::Rc::new(teoremas),
        numeros_teoremas: std::rc::Rc::new(std::cell::RefCell::new(numeros_teoremas)),
        caixas: std::rc::Rc::new(pacotes::caixas_do_preambulo(preambulo)),
        listas_novas: std::rc::Rc::new(listas_novas(preambulo)),
        // O `listings` do pdfLaTeX não aceita letra acentuada sem `literate` (no XeLaTeX/LuaLaTeX, sim).
        estilos_lst: std::rc::Rc::new(estilos_lst(preambulo)),
        tikz: std::rc::Rc::new(tikz::definicoes(preambulo)),
        tipos_de_coluna: std::rc::Rc::new(tipos_de_coluna(preambulo)),
        rotulos_do_preambulo: preambulo.contains("\\labelitem") || preambulo.contains("\\labelenum") || preambulo.match_indices("\\setlist").any(|(i, _)| {
            let r = &preambulo[i + 8..];
            let (alvo, k) = opcional(r).unwrap_or(("", 0));
            (alvo.is_empty() || alvo.contains("itemize") || alvo.contains("enumerate")) && grupo(&r[k..]).is_some_and(|(g, _)| g.contains("label"))
        }),
        estilo_url: preambulo.rfind("\\urlstyle").and_then(|i| grupo(&preambulo[i + 9..])).map_or(String::new(), |(g, _)| g.trim().to_string()),
        listings_utf8: ["literate", "inputencoding", "listingsutf8", "fontspec"].iter().any(|c| preambulo.contains(c)),
        tabulacao: preambulo.find("tabsize=").and_then(|p| preambulo[p + 8..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()).filter(|t| *t > 0).unwrap_or(8),
        titulo_fonte: fonte_do_argumento(preambulo, "\\title").unwrap_or_default().to_string(),
        autor_fonte: fonte_do_argumento(preambulo, "\\author").unwrap_or_default().to_string(),
        data_fonte: fonte_do_argumento(preambulo, "\\date").map(str::to_string),
        espaco: espaco_do(preambulo),
        largura_util: doc.pagina.largura_util(),
        legenda_negrito,
        estica_tabela: estica_do(preambulo),
        // Só o `\raggedright` solto vale para o documento; dentro de `{…}` é de uma coluna ou de um comando.
        alinhamento: if ["\\raggedright", "\\RaggedRight"].iter().any(|c| fora_de_grupo(preambulo, c)) { Alinhamento::Esquerda } else { Alinhamento::Justificado },
        avisos: coletor.clone(),
        ..Ctx::default()
    };
    let capitulo = ctx.capitulo;
    let formatos = formatos_dos_titulos(preambulo, ctx.classe, &ctx.cores, capitulo);
    let cores_do_documento = ctx.cores.clone();
    let recuo = recuo_de_paragrafo(preambulo, classe);
    // O bloco com `\label` chega com um id provisório; o `\ref` aponta para `#rotulo:<nome>`.
    let mut destinos: HashMap<String, u64> = HashMap::new();
    let mut topo = Analisador::novo(&corpo_expandido, ctx);
    topo.recuo = recuo;
    for mut b in topo.executar() {
        if b.id & avisos::MARCA_DE_ROTULO != 0 {
            let indice = (b.id & !avisos::MARCA_DE_ROTULO) as usize;
            for nome in coletor.0.borrow().rotulos.get(indice).into_iter().flatten() {
                destinos.entry(nome.clone()).or_insert(doc.proximo_id);
            }
        }
        b.id = doc.proximo_id;
        doc.proximo_id += 1;
        doc.blocos.push(b);
    }
    for t in doc.blocos.iter_mut().flat_map(|b| b.conteudo.textos_mut()).flatten() {
        if let Some(nome) = t.link.as_deref().and_then(|l| l.strip_prefix("#rotulo:")) {
            t.link = destinos.get(nome).map(|id| format!("#bloco:{id}"));
        }
    }
    cores_dos_links(&mut doc, preambulo, &cores_do_documento);
    numerar_como_o_fonte(&mut doc, &corpo_expandido, capitulo);
    titulos_em_maiusculas(&mut doc, preambulo, capitulo);
    for b in &mut doc.blocos {
        if let Conteudo::Titulo { nivel, .. } = b.conteudo {
            b.latex = formatos[usize::from(nivel.clamp(1, 6))].clone();
        }
    }
    titulos_em_bloco(&mut doc, preambulo, capitulo);
    if corpo_expandido.contains("\\nocite{*}") {
        for b in &mut doc.blocos {
            if let Conteudo::Bibliografia { todas } = &mut b.conteudo {
                *todas = true;
            }
        }
    } else if corpo_expandido.contains("\\citeauthor") || corpo_expandido.contains("\\citeyear") {
        // O `\citeauthor`/`\citeyear` viram texto (sem a marca de citação), então a lista não as
        // veria: fica a lista toda, só com as fontes que o código cita, como o BibTeX faria.
        let citadas = chaves_no_codigo(&corpo_expandido);
        doc.bibliografia.fontes.retain(|f| citadas.contains(&f.chave));
        for b in &mut doc.blocos {
            if let Conteudo::Bibliografia { todas } = &mut b.conteudo {
                *todas = true;
            }
        }
    }
    doc
}

/// As chaves de todo `\cite…{a,b}` do código (`\cite`, `\citeauthor`, `\citeonline`, `\nocite`…).
fn chaves_no_codigo(corpo: &str) -> std::collections::HashSet<String> {
    let mut v = std::collections::HashSet::new();
    let mut resto = corpo;
    while let Some(i) = [resto.find("\\cite"), resto.find("\\nocite")].into_iter().flatten().min() {
        let depois = &resto[i + 1..];
        let Some(a) = depois.find('{') else { break };
        // Só letras entre o nome e a chave (o `[p. 4]` da página é pulado).
        let nome_ok = depois[..a].split('[').next().is_some_and(|n| n.chars().all(|c| c.is_ascii_alphabetic() || c == '*'));
        let Some(f) = depois[a..].find('}') else { break };
        if nome_ok {
            v.extend(depois[a + 1..a + f].split(',').map(|k| k.trim().to_string()));
        }
        resto = &depois[a + f..];
    }
    v
}

/// As fontes do `filecontents` (o `.bib` dentro do próprio `.tex`) e o estilo do `biblatex` ou do
/// `\bibliographystyle`. Um `.bib` de fora não dá para ler daqui (o leitor só recebe o texto).
fn ler_fontes(doc: &mut Documento, fonte: &str, preambulo: &str) {
    for abre in ["\\begin{filecontents*}", "\\begin{filecontents}"] {
        if let Some(i) = fonte.find(abre) {
            let depois = &fonte[i + abre.len()..];
            let fim = depois.find("\\end{filecontents").unwrap_or(depois.len());
            doc.bibliografia.fontes.extend(crate::bib::ler(&depois[..fim]));
        }
    }
    let p = preambulo.to_lowercase();
    let estilo = if p.contains("biblatex-chicago") || p.contains("style=chicago") {
        Some(EstiloDeCitacao::Chicago)
    } else if p.contains("style=abnt") || p.contains("abntex2") {
        Some(EstiloDeCitacao::Abnt)
    } else if p.contains("style=apa") || p.contains("{apalike}") || p.contains("{apa}") {
        Some(EstiloDeCitacao::Apa)
    } else if p.contains("style=ieee") || p.contains("{ieeetr}") || p.contains("{ieeetran}") {
        Some(EstiloDeCitacao::Ieee)
    } else if p.contains("style=vancouver") || p.contains("{vancouver}") || p.contains("{unsrt}") {
        Some(EstiloDeCitacao::Vancouver)
    } else {
        None
    };
    if let Some(e) = estilo {
        doc.bibliografia.estilo = e;
    }
    doc.bibliografia.sem_sl_sn = p.contains("noslsn");
}

/// `\section{…}` numera e `\section*{…}` não: com alguma divisão sem estrela, a numeração dos títulos
/// liga (de capítulos, se há `\chapter`) e os títulos com estrela ficam fora dela. As divisões do fonte
/// e os títulos do documento saem na mesma ordem.
fn numerar_como_o_fonte(doc: &mut Documento, corpo: &str, capitulo: bool) {
    let estrelas: Vec<bool> = corpo
        .match_indices('\\')
        .filter_map(|(i, _)| {
            let (nome, n) = comando(&corpo[i..]);
            // O título que o `\printbibliography[title=…]` escreve não tem número.
            if nome == "printbibliography" {
                return opcional(&corpo[i + n..]).and_then(|(o, _)| titulo_da_bibliografia(o)).map(|_| true);
            }
            DIVISOES.iter().any(|d| d.0 == nome).then(|| corpo[i..i + n].ends_with('*'))
        })
        .collect();
    if estrelas.iter().all(|e| *e) {
        return;
    }
    doc.estilos.numeracao = if capitulo { Numeracao::Capitulo } else { Numeracao::Decimal };
    // O `\appendix`: o primeiro título depois dele marca onde as letras começam.
    let e_divisao = |i: usize| DIVISOES.iter().any(|d| d.0 == comando(&corpo[i..]).0);
    let apendice = corpo.match_indices('\\').find(|(i, _)| comando(&corpo[*i..]).0 == "appendix").map(|(p, _)| corpo[..p].match_indices('\\').filter(|(i, _)| e_divisao(*i)).count());
    let titulos = doc.blocos.iter_mut().filter(|b| matches!(b.conteudo, Conteudo::Titulo { .. }));
    for (k, (b, estrela)) in titulos.zip(estrelas).enumerate() {
        b.atrib.sem_numero = estrela;
        b.atrib.apendice = Some(k) == apendice;
    }
}

/// Um comprimento do TeX em pontos (`8pt`, `13mm`, `1.4em`, `-2pt`); `None` se não é um.
fn dimensao(s: &str, classe: Classe) -> Option<f32> {
    let s = s.trim();
    let fim = s.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == ',')).unwrap_or(s.len());
    let n: f32 = s[..fim].replace(',', ".").parse().ok()?;
    let corpo = if classe.corpo > 0.0 { classe.corpo } else { 10.0 };
    let unidade = s[fim..].trim_start();
    let fator = match unidade.get(..2).unwrap_or(unidade) {
        "pt" | "bp" | "" => 1.0,
        "mm" => 72.0 / 25.4,
        "cm" => 72.0 / 2.54,
        "in" => 72.0,
        "em" => corpo,
        "ex" => corpo * 0.43,
        "pc" => 12.0,
        _ => return None,
    };
    Some(n * fator)
}

/// `\setlength{\nome}{valor}` no preâmbulo.
fn comprimento_definido(preambulo: &str, nome: &str, classe: Classe) -> Option<f32> {
    let mut resto = preambulo;
    let mut achado = None;
    while let Some(i) = resto.find("\\setlength") {
        resto = &resto[i + 10..];
        if let Some((alvo, k)) = grupo(resto) {
            if alvo.trim() == nome {
                achado = grupo(&resto[k..]).and_then(|(v, _)| dimensao(v, classe));
            }
        }
    }
    achado
}

/// O `\parindent` (15, 17 ou 18 pt conforme o corpo da classe; 1,3 cm no abnTeX2; 0 com o pacote
/// `parskip`) e se o primeiro parágrafo depois de um título recua também (`indentfirst`, abnTeX2).
fn recuo_de_paragrafo(preambulo: &str, classe: Classe) -> (f32, bool) {
    let abntex = avisos::classe_do_preambulo(preambulo).is_some_and(|c| c.starts_with("abntex2"));
    let padrao = if abntex {
        1.3 * 72.0 / 2.54
    } else if preambulo.contains("{parskip}") {
        0.0
    } else {
        match classe.tamanhos[4] as u32 {
            12 => 18.0,
            10 if classe.tamanhos[4] > 10.5 => 17.0,
            _ => 15.0,
        }
    };
    (comprimento_definido(preambulo, "\\parindent", classe).unwrap_or(padrao), abntex || preambulo.contains("indentfirst"))
}

/// O argumento de `\comando[opções]{argumento}` (a última vez que aparece), pulando as opções.
fn fonte_do_argumento_opcional<'a>(texto: &'a str, comando: &str) -> Option<&'a str> {
    let i = texto.rfind(comando)?;
    let resto = &texto[i + comando.len()..];
    let o = opcional(resto).map_or(0, |(_, k)| k);
    grupo(&resto[o..]).map(|(g, _)| g.trim())
}

/// As opções `chave=valor` de um `[…]` ou `{…}`.
fn opcoes_chave_valor(s: &str) -> Vec<(String, String)> {
    let mut saida = Vec::new();
    let (mut prof, mut atual) = (0, String::new());
    for c in s.chars().chain(std::iter::once(',')) {
        match c {
            '{' => prof += 1,
            '}' => prof -= 1,
            ',' if prof == 0 => {
                let (k, v) = atual.split_once('=').unwrap_or((atual.as_str(), ""));
                saida.push((k.trim().to_string(), v.trim().to_string()));
                atual.clear();
                continue;
            }
            _ => {}
        }
        atual.push(c);
    }
    saida
}

/// Papel e margens (`geometry`, `a4paper`/`letterpaper`) e o estilo de página (`\pagestyle{empty}`).
fn configurar_pagina(doc: &mut Documento, preambulo: &str) {
    let classe = Classe::do_preambulo(preambulo);
    let mut opcoes = Vec::new();
    if let Some(i) = preambulo.find("\\documentclass") {
        opcoes.extend(opcional(&preambulo[i + 14..]).map(|(o, _)| opcoes_chave_valor(o)).unwrap_or_default());
    }
    for (marca, abre) in [("{geometry}", None), ("\\geometry", Some('{'))] {
        let Some(i) = preambulo.find(marca) else { continue };
        let texto = match abre {
            Some(_) => grupo(&preambulo[i + marca.len()..]).map(|(g, _)| g),
            // `\usepackage[opções]{geometry}`: as opções vêm antes do nome.
            None => preambulo[..i].rfind('[').map(|a| &preambulo[a + 1..preambulo[..i].rfind(']').unwrap_or(a + 1).max(a + 1)]),
        };
        opcoes.extend(texto.map(opcoes_chave_valor).unwrap_or_default());
    }
    let p = &mut doc.pagina;
    // A abnTeX2 já vem com as margens da ABNT: 3 cm em cima e à esquerda, 2 cm embaixo e à direita.
    if avisos::classe_do_preambulo(preambulo).as_deref() == Some("abntex2") {
        let (tres, dois) = (3.0 / 2.54 * 72.0, 2.0 / 2.54 * 72.0);
        (p.margem_sup, p.margem_esq, p.margem_inf, p.margem_dir) = (tres, tres, dois, dois);
    }
    let mut deitado = false;
    for (k, v) in opcoes {
        let d = || dimensao(&v, classe);
        match k.as_str() {
            "a4paper" => (p.largura, p.altura) = (595.28, 841.89),
            "letterpaper" => (p.largura, p.altura) = (612.0, 792.0),
            "paperwidth" => p.largura = d().unwrap_or(p.largura),
            "paperheight" => p.altura = d().unwrap_or(p.altura),
            "landscape" => deitado = true,
            "top" | "tmargin" => p.margem_sup = d().unwrap_or(p.margem_sup),
            "bottom" | "bmargin" => p.margem_inf = d().unwrap_or(p.margem_inf),
            "left" | "lmargin" | "inner" => p.margem_esq = d().unwrap_or(p.margem_esq),
            "right" | "rmargin" | "outer" => p.margem_dir = d().unwrap_or(p.margem_dir),
            "margin" => {
                if let Some(m) = d() {
                    (p.margem_sup, p.margem_inf, p.margem_esq, p.margem_dir) = (m, m, m, m);
                }
            }
            "hmargin" => {
                if let Some(m) = d() {
                    (p.margem_esq, p.margem_dir) = (m, m);
                }
            }
            "vmargin" => {
                if let Some(m) = d() {
                    (p.margem_sup, p.margem_inf) = (m, m);
                }
            }
            _ => {}
        }
    }
    if deitado && p.largura < p.altura {
        (p.largura, p.altura) = (p.altura, p.largura);
    }
    let estilo = preambulo.find("\\pagestyle").and_then(|i| grupo(&preambulo[i + 10..])).map(|(g, _)| g.trim().to_string());
    if matches!(estilo.as_deref(), Some("empty")) {
        doc.extras.cabecalho = Faixas::default();
        doc.extras.rodape = Faixas::default();
    }
}

/// O resto do que vale para a página: cor do papel (`\pagecolor`), marca d'água (`draftwatermark`),
/// números de linha (`lineno`), capa sem cabeçalho (`\thispagestyle{empty}` no começo), o texto
/// no meio ou embaixo da página (`\vspace*{\fill}`) e a forma do número das notas.
fn extras_da_pagina(doc: &mut Documento, preambulo: &str, corpo: &str, cores: &HashMap<String, u32>) {
    let x = &mut doc.extras;
    let cor_de = |cmd: &str, onde: &str| -> Option<u32> {
        let i = onde.find(cmd)?;
        let depois = &onde[i + cmd.len()..];
        match opcional(depois) {
            Some((m, k)) => parsear_cor(m, grupo(&depois[k..])?.0),
            None => resolver_cor(cores, grupo(depois)?.0),
        }
    };
    let tudo = format!("{preambulo}{corpo}");
    if let Some(c) = cor_de("\\pagecolor", &tudo) {
        x.cor_pagina = c;
    }
    if let Some(i) = tudo.find("\\SetWatermarkText") {
        x.marca = grupo(&tudo[i + "\\SetWatermarkText".len()..]).map(|(g, _)| texto_puro(&inline(g, cores)).trim().to_string()).unwrap_or_default();
        x.marca_cor = cor_de("\\SetWatermarkColor", &tudo).unwrap_or(0);
    }
    if tudo.contains("\\pagewiselinenumbers") {
        x.numerar_linhas = featherdocs_core::modelo::NumeracaoDeLinhas::PorPagina;
    } else if tudo.contains("\\linenumbers") {
        x.numerar_linhas = featherdocs_core::modelo::NumeracaoDeLinhas::Continua;
    }
    // O começo do corpo, depois do que o FeatherDocs escreve antes do texto.
    let mut inicio = corpo.trim_start();
    while let Some((c, r)) = ["\\maketitle", "\\thispagestyle{empty}", "\\linenumbers", "\\pagewiselinenumbers"].iter().find_map(|c| inicio.strip_prefix(c).map(|r| (*c, r))) {
        x.primeira_diferente |= c == "\\thispagestyle{empty}";
        inicio = r.trim_start();
    }
    let fill = "\\vspace*{\\fill}";
    if inicio.starts_with(fill) {
        x.vertical = if corpo.trim_end().ends_with(fill) { Vertical::Centro } else { Vertical::Base };
    }
    for (forma, n) in [("\\alph{footnote}", 1), ("\\roman{footnote}", 2), ("\\fnsymbol{footnote}", 3)] {
        if preambulo.contains(&format!("\\renewcommand{{\\thefootnote}}{{{forma}}}")) {
            x.formato_notas = n;
        }
    }
}

/// O `fancyhdr`: com `\pagestyle{fancy}` (no preâmbulo ou no corpo), o `\fancyhead[R]{…}`, o
/// `\fancyfoot[C]{…}`, o `\lhead{…}`… viram o cabeçalho e o rodapé; `\thepage` é o número da página.
/// Um estilo do `\fancypagestyle{nome}{…}` vale quando o `\pagestyle{nome}` o escolhe, e o do
/// `\thispagestyle{nome}` fica só na primeira página.
fn cabecalho_e_rodape(doc: &mut Documento, preambulo: &str, corpo: &str, cores: &HashMap<String, u32>, macros: &HashMap<String, MacroDef>, classe: Classe) {
    let tudo = format!("{preambulo}{corpo}");
    let definicoes = estilos_do_fancy(&tudo);
    // O `\pagestyle` de dentro de um `\fancypagestyle` não escolhe nada.
    let fora = definicoes.iter().fold(tudo.clone(), |t, (_, def)| t.replace(def.as_str(), ""));
    let memoir = estilos_do_memoir(&tudo);
    // No memoir, sem `\pagestyle`, vale o `headings`.
    let padrao = (!memoir.is_empty()).then(|| "headings".to_string());
    let definicoes = definicoes.into_iter().chain(memoir);
    // O mesmo nome definido de novo (o `.tex` pondo o logo no estilo da classe) soma ao anterior.
    let mut estilos: HashMap<String, String> = HashMap::new();
    for (nome, def) in definicoes {
        estilos.entry(nome).or_default().push_str(&def);
    }
    let estilo = |nome: &str| fora.find(nome).and_then(|i| grupo(&fora[i + nome.len()..])).map(|(g, _)| g.trim().to_string());
    let linha = doc.pagina.largura - doc.pagina.margem_esq - doc.pagina.margem_dir;
    let imagens = imagens_declaradas(&tudo);
    let x = &mut doc.extras;
    match estilo("\\pagestyle").or(padrao).as_deref() {
        Some("empty") => {
            x.cabecalho = Faixas::default();
            x.rodape = Faixas::default();
            return;
        }
        Some(nome) if estilos.contains_key(nome) => {
            (x.cabecalho, x.rodape) = faixas_do_fancy(&estilos[nome], cores, macros, classe, &imagens, linha);
        }
        Some("fancy") => (x.cabecalho, x.rodape) = faixas_do_fancy(&fora, cores, macros, classe, &imagens, linha),
        // Os da classe `exam` (`\\lfoot{…}`, `\\rfoot{…}`): sem fio, a não ser com `\\runningheadrule`.
        Some("headandfoot" | "head" | "foot") => {
            (x.cabecalho, x.rodape) = faixas_do_fancy(&fora, cores, macros, classe, &imagens, linha);
            x.cabecalho.fio = if fora.contains("\\runningheadrule") { 0.4 } else { 0.0 };
        }
        // O `plain` (o padrão): só o número no pé, na letra do texto.
        _ => letra_do_texto(&mut x.rodape, classe),
    }
    if let Some(def) = estilo("\\thispagestyle").and_then(|n| estilos.get(&n)) {
        (x.cabecalho_primeira, x.rodape_primeira) = faixas_do_fancy(def, cores, macros, classe, &imagens, linha);
        x.primeira_diferente = true;
    }
}

/// Os `\fancypagestyle{nome}{definição}`, na ordem do texto.
fn estilos_do_fancy(texto: &str) -> Vec<(String, String)> {
    let mut estilos = Vec::new();
    let mut resto = texto;
    while let Some(i) = resto.find("\\fancypagestyle") {
        resto = &resto[i + "\\fancypagestyle".len()..];
        let Some((nome, k)) = grupo(resto) else { continue };
        let Some((def, j)) = grupo(&resto[k..]) else { continue };
        estilos.push((nome.trim().to_string(), def.to_string()));
        resto = &resto[k + j..];
    }
    estilos
}

/// Os estilos do memoir (`\makepagestyle{nome}`, `\makeoddhead{nome}{esq}{centro}{dir}`,
/// `\makeoddfoot`) escritos como os do `fancyhdr`, na ordem do texto. A página par (`\makeevenhead`)
/// fica de fora: o documento não tem página par diferente.
fn estilos_do_memoir(texto: &str) -> Vec<(String, String)> {
    let mut estilos = Vec::new();
    for (i, _) in texto.match_indices("\\make") {
        let resto = &texto[i + 5..];
        let Some((cmd, r)) = ["pagestyle", "oddhead", "oddfoot"].iter().find_map(|c| resto.strip_prefix(c).map(|r| (*c, r))) else { continue };
        let Some((nome, k)) = grupo(r) else { continue };
        let nome = nome.trim().to_string();
        // Na primeira vez do nome, o estilo começa vazio e sem o fio sob o cabeçalho (o memoir não põe).
        if !estilos.iter().any(|(n, _)| *n == nome) {
            estilos.push((nome.clone(), "\\fancyhf{}\\renewcommand{\\headrulewidth}{0pt}".to_string()));
        }
        if cmd == "pagestyle" {
            continue;
        }
        let mut r = &r[k..];
        let mut campos = String::new();
        let faixa = if cmd == "oddhead" { "\\fancyhead" } else { "\\fancyfoot" };
        for lado in ["L", "C", "R"] {
            let Some((g, k)) = grupo(r) else { break };
            campos.push_str(&format!("{faixa}[{lado}]{{{g}}}"));
            r = &r[k..];
        }
        estilos.push((nome, campos));
    }
    estilos
}

/// O `\pgfdeclareimage[opções]{nome}{arquivo}`: o arquivo e as opções de cada nome.
fn imagens_declaradas(texto: &str) -> HashMap<String, (String, String)> {
    let mut imagens = HashMap::new();
    let mut resto = texto;
    while let Some(i) = resto.find("\\pgfdeclareimage") {
        resto = &resto[i + "\\pgfdeclareimage".len()..];
        let (opcoes, o) = opcional(resto).unwrap_or(("", 0));
        let Some((nome, k)) = grupo(&resto[o..]) else { continue };
        let Some((arquivo, j)) = grupo(&resto[o + k..]) else { continue };
        imagens.insert(nome.trim().to_string(), (arquivo.trim().to_string(), opcoes.to_string()));
        resto = &resto[o + k + j..];
    }
    imagens
}

/// O cabeçalho e o rodapé que os comandos do `fancyhdr` em `texto` montam (o documento todo ou a
/// definição de um `\fancypagestyle`). `linha` é a largura do texto, para o `width=1.27\linewidth`.
fn faixas_do_fancy(texto: &str, cores: &HashMap<String, u32>, macros: &HashMap<String, MacroDef>, classe: Classe, imagens: &HashMap<String, (String, String)>, linha: f32) -> (Faixas, Faixas) {
    // O `fancy` sem nada definido: o número no centro do rodapé.
    let mut cabecalho = Faixas::default();
    let mut rodape = Faixas { centro: "{pagina}".to_string(), ..Faixas::default() };
    if texto.contains("\\fancyhf{}") {
        rodape = Faixas::default();
    }
    // O texto do campo e o tamanho da letra dele (`\begin{tiny}…\end{tiny}`, `{\footnotesize …}`).
    let em_texto = |g: &str| -> (String, Option<f32>) {
        // Os campos passam pelo leitor como caracteres privados: as chaves de `{pagina}` seriam um grupo,
        // e a expansão leria o `\thepage` como contador.
        let mut g = g.replace("\\thepage", "\u{E000}").replace("\\pageref{LastPage}", "\u{E001}").replace("\\numpages", "\u{E001}");
        let mut tamanho = None;
        for nome in DECLARACOES_DE_TAMANHO {
            let antes = g.len();
            g = g.replace(&format!("\\begin{{{nome}}}"), "").replace(&format!("\\end{{{nome}}}"), "");
            let cmd = format!("\\{nome}");
            while let Some(i) = g.match_indices(&cmd).map(|(i, _)| i).find(|&i| !g[i + cmd.len()..].starts_with(|c: char| c.is_alphabetic())) {
                g.replace_range(i..i + cmd.len(), "");
            }
            if g.len() != antes {
                tamanho = classe.tamanho_de(nome).filter(|t| *t > 0.0);
            }
        }
        let g = expandir_macros(g, macros);
        (texto_puro(&inline(&g, cores)).trim().replace('\u{E000}', "{pagina}").replace('\u{E001}', "{total}"), tamanho)
    };
    // `\pgfuseimage{logo}` ou `\includegraphics[width=…]{arquivo}`: o arquivo, a largura e a altura
    // (0 = da proporção da imagem), e o texto sem a imagem.
    let imagem_de = |g: &str| -> Option<(String, f32, f32, String)> {
        // O `\pgfdeclareimage` mede no preâmbulo, onde o `\linewidth` ainda é o da classe (o `geometry`
        // só muda a página no `\begin{document}`): 345, 360 ou 390 pt no corpo 10, 11 ou 12.
        let (arquivo, opcoes, inicio, fim, linha) = if let Some(i) = g.find("\\pgfuseimage") {
            let (nome, k) = grupo(&g[i + 12..])?;
            let (arquivo, opcoes) = imagens.get(nome.trim())?;
            let da_classe = match classe.tamanhos[4] {
                12.0 => 390.0,
                10.95 => 360.0,
                _ => 345.0,
            };
            (arquivo.clone(), opcoes.clone(), i, i + 12 + k, da_classe)
        } else {
            let i = g.find("\\includegraphics")?;
            let depois = &g[i + 16..];
            let (opcoes, o) = opcional(depois).unwrap_or(("", 0));
            let (arquivo, k) = grupo(&depois[o..])?;
            (arquivo.trim().to_string(), opcoes.to_string(), i, i + 16 + o + k, linha)
        };
        let medida = |chave: &str| {
            opcoes_chave_valor(&opcoes).into_iter().find(|(k, _)| k == chave).map_or(0.0, |(_, v)| {
                let v = v.trim();
                ["\\linewidth", "\\textwidth", "\\columnwidth"].iter().find_map(|l| v.strip_suffix(l)).map_or_else(|| dimensao(v, classe).unwrap_or(0.0), |f| f.trim().parse::<f32>().unwrap_or(1.0) * linha)
            })
        };
        Some((arquivo, medida("width"), medida("height"), format!("{}{}", &g[..inicio], &g[fim..])))
    };
    let (mut tamanho_cabecalho, mut tamanho_rodape) = (None, None);
    for (cmd, no_rodape, lados) in [("\\fancyhead", false, ""), ("\\fancyfoot", true, ""), ("\\lhead", false, "L"), ("\\chead", false, "C"), ("\\rhead", false, "R"), ("\\lfoot", true, "L"), ("\\cfoot", true, "C"), ("\\rfoot", true, "R")] {
        let mut resto = texto;
        while let Some(i) = resto.find(cmd) {
            let mut depois = &resto[i + cmd.len()..];
            if depois.starts_with(|c: char| c.is_alphabetic()) {
                resto = depois;
                continue;
            }
            let mut quais = lados.to_string();
            if let Some((o, k)) = opcional(depois) {
                quais = o.to_uppercase();
                depois = &depois[k..];
            } else if quais.is_empty() {
                quais = "LCR".to_string();
            }
            let Some((g, k)) = grupo(depois) else { break };
            let f = if no_rodape { &mut rodape } else { &mut cabecalho };
            let (t, tamanho) = match imagem_de(g) {
                Some((arquivo, w, h, sem)) => {
                    (f.imagem, f.imagem_largura, f.imagem_altura) = (arquivo, w, h);
                    em_texto(&sem)
                }
                None => em_texto(g),
            };
            if tamanho.is_some() {
                *(if no_rodape { &mut tamanho_rodape } else { &mut tamanho_cabecalho }) = tamanho;
            }
            // `[LE,RO]`: só o lado (o documento não tem página par diferente).
            for lado in quais.chars() {
                match lado {
                    'L' => f.esquerda = t.clone(),
                    'C' => f.centro = t.clone(),
                    'R' => f.direita = t.clone(),
                    _ => {}
                }
            }
            resto = &depois[k..];
        }
    }
    // Como o `fancyhdr` desenha: a letra do texto, em preto, e o fio de 0,4 pt sob o cabeçalho
    // (o do rodapé é 0) que o `\renewcommand{\headrulewidth}` muda.
    let fio = |nome: &str, padrao: f32| {
        let chave = format!("\\renewcommand{{\\{nome}}}");
        texto.rfind(&chave).and_then(|i| grupo(&texto[i + chave.len()..])).and_then(|(g, _)| dimensao(g, classe)).unwrap_or(padrao)
    };
    for (f, nome, padrao, tamanho) in [(&mut cabecalho, "headrulewidth", 0.4, tamanho_cabecalho), (&mut rodape, "footrulewidth", 0.0, tamanho_rodape)] {
        f.fio = fio(nome, padrao);
        letra_do_texto(f, classe);
        // shortcut: um tamanho só por faixa (o último campo com tamanho), os três lados juntos.
        f.tamanho = tamanho.unwrap_or(f.tamanho);
    }
    (cabecalho, rodape)
}

/// A faixa na letra do corpo do texto, em preto (o cabeçalho e o rodapé do LaTeX).
fn letra_do_texto(f: &mut Faixas, classe: Classe) {
    f.estilo = classe.familia;
    f.tamanho = if classe.corpo > 0.0 { classe.corpo } else { 10.0 };
    f.cor = 0x000000FF;
}

/// O `\setlist[itemize]{…}` do `enumitem`: o espaço entre itens e a cor do marcador (`label=\textcolor{x}{…}`).
/// `\newlist{nome}{enumerate}{n}` e os `\setlist[nome,k]{opções}` dele: numerada e as opções por nível.
fn listas_novas(preambulo: &str) -> HashMap<String, (bool, Vec<String>)> {
    let mut listas: HashMap<String, (bool, Vec<String>)> = HashMap::new();
    let mut resto = preambulo;
    while let Some(i) = resto.find("\\newlist") {
        resto = &resto[i + 8..];
        let Some((nome, k1)) = grupo(resto) else { continue };
        let Some((tipo, k2)) = grupo(&resto[k1..]) else { continue };
        let niveis = grupo(&resto[k1 + k2..]).and_then(|(n, _)| n.trim().parse::<usize>().ok()).unwrap_or(1).clamp(1, 9);
        listas.insert(nome.trim().to_string(), (tipo.trim() == "enumerate", vec![String::new(); niveis]));
    }
    let mut resto = preambulo;
    while let Some(i) = resto.find("\\setlist[") {
        resto = &resto[i + 8..];
        let Some((alvo, k)) = opcional(resto) else { continue };
        let Some((opcoes, _)) = grupo(&resto[k..]) else { continue };
        let (nome, nivel) = alvo.split_once(',').map_or((alvo, None), |(n, k)| (n, k.trim().parse::<usize>().ok()));
        if let Some((_, niveis)) = listas.get_mut(nome.trim()) {
            for (k, o) in niveis.iter_mut().enumerate() {
                if nivel.is_none_or(|n| n == k + 1) {
                    *o = opcoes.to_string();
                }
            }
        }
    }
    listas
}

/// O `label=` do `enumitem` como rótulo do FeatherDocs: o tipo de lista e, se não for um dos simples
/// (1. a) I. i.), o modelo de texto para `Bloco::latex` (ver `paginacao::rotulo_do_modelo`). `resume`
/// continua a contagem entre listas.
fn modelo_de_rotulo(rotulo: &str, resume: bool, alinhado: bool) -> (featherdocs_core::modelo::Lista, Option<String>) {
    use featherdocs_core::modelo::Lista;
    let negrito = rotulo.contains("\\textbf") || rotulo.contains("\\bfseries");
    let mut m = rotulo.replace("\\textbf", "").replace("\\bfseries", "");
    for (de, para) in [("\\arabic*", "#1"), ("\\alph*", "#a"), ("\\Alph*", "#A"), ("\\roman*", "#i"), ("\\Roman*", "#I"), ("\\ordcard*", "#o"), ("~", " "), ("\\,", " "), ("\\ ", " ")] {
        m = m.replace(de, para);
    }
    let m: String = m.chars().filter(|c| !matches!(c, '{' | '}')).collect();
    let m = m.trim();
    let lista = if m.contains("#a") || m.contains("#A") {
        Lista::Alfabetica
    } else if m.contains("#I") {
        Lista::Romana
    } else if m.contains("#i") {
        Lista::RomanaMinuscula
    } else if m.contains('#') {
        Lista::Numerica
    } else {
        Lista::Padrao
    };
    // `align=left` põe o rótulo no começo do nível: só o caminho do rótulo de texto faz isso.
    let simples = !alinhado && !negrito && !resume && matches!(m, "#1." | "#a)" | "#I." | "#i.");
    let entende = !m.is_empty() && !m.contains(['\\', '$']);
    let estilo = format!("{}{}{}", if negrito { "b" } else { "" }, if resume { "r" } else { "" }, if alinhado { "" } else { "d" });
    (lista, (entende && !simples).then(|| format!("{estilo}|{m}")))
}

fn listas_do_preambulo(preambulo: &str, cores: &HashMap<String, u32>, classe: Classe) -> ([f32; 2], [u32; 2]) {
    // O vão entre itens é `\itemsep` + `\parsep`: 4 pt + 4 pt no corpo 10, 4,5 + 4,5 no 11, 5 + 5 no 12.
    let corpo = classe.tamanhos[4];
    let padrao = if corpo >= 12.0 { 5.0 } else if corpo > 10.0 { 4.5 } else { 4.0 };
    // [itemize, enumerate]: (itemsep, parsep).
    let mut vaos = [(padrao, padrao); 2];
    // A cor do marcador também é por tipo: `\setlist[itemize]{label=\textcolor{…}}` não pinta o enumerate.
    let mut cor = [0u32; 2];
    for (i, _) in preambulo.match_indices("\\setlist") {
        let resto = &preambulo[i + 8..];
        let alvo = opcional(resto).map(|(o, _)| o.trim().to_string());
        let resto = opcional(resto).map_or(resto, |(_, k)| &resto[k..]);
        let Some((g, _)) = grupo(resto) else { continue };
        for (k, (itemsep, parsep)) in vaos.iter_mut().enumerate() {
            if alvo.as_deref().is_some_and(|a| !a.split(',').any(|a| a.trim() == ["itemize", "enumerate"][k])) {
                continue;
            }
            for (chave, v) in opcoes_chave_valor(g) {
                match chave.as_str() {
                    "itemsep" => *itemsep = dimensao(&v, classe).unwrap_or(*itemsep),
                    "parsep" => *parsep = dimensao(&v, classe).unwrap_or(*parsep),
                    "noitemsep" | "nosep" => (*itemsep, *parsep) = (0.0, 0.0),
                    _ => {}
                }
            }
        }
        for (k, v) in opcoes_chave_valor(g) {
            if k != "label" {
                continue;
            }
            let achada = if let Some(j) = v.find("\\textcolor") {
                grupo(&v[j + 10..]).and_then(|(c, _)| resolver_cor(cores, c)).unwrap_or(0)
            } else if let Some(j) = v.find("\\color") {
                grupo(&v[j + 6..]).and_then(|(c, _)| resolver_cor(cores, c)).unwrap_or(0)
            } else {
                continue;
            };
            for (t, c) in cor.iter_mut().enumerate() {
                if alvo.as_deref().is_none_or(|a| a.split(',').any(|a| a.trim() == ["itemize", "enumerate"][t])) {
                    *c = achada;
                }
            }
        }
    }
    (vaos.map(|(a, b)| a + b), cor)
}

/// Corta o que vem depois de um `%` não escapado, linha a linha.
fn sem_comentarios(fonte: &str) -> String {
    let mut saida = String::with_capacity(fonte.len());
    // Dentro de um código (`lstlisting`, `verbatim`, `minted`) o `%` é texto.
    let mut codigo: Option<&str> = None;
    for linha in fonte.lines() {
        if let Some(fim) = codigo {
            if linha.contains(fim) {
                codigo = None;
            } else {
                saida.push_str(linha);
                saida.push('\n');
                continue;
            }
        } else if let Some(amb) = ["lstlisting", "verbatim", "minted", "Verbatim"].into_iter().find(|a| linha.contains(&format!("\\begin{{{a}}}")) && !linha.contains(&format!("\\end{{{a}}}"))) {
            codigo = Some(match amb {
                "lstlisting" => "\\end{lstlisting}",
                "verbatim" => "\\end{verbatim}",
                "minted" => "\\end{minted}",
                _ => "\\end{Verbatim}",
            });
        }
        let mut anterior = ' ';
        let mut fim = linha.len();
        for (i, c) in linha.char_indices() {
            if c == '%' && anterior != '\\' {
                fim = i;
                break;
            }
            anterior = c;
        }
        saida.push_str(&linha[..fim]);
        saida.push('\n');
    }
    saida
}

fn separar(src: &str) -> (&str, &str) {
    let Some(ini) = src.find("\\begin{document}") else { return ("", src) };
    let corpo = &src[ini + "\\begin{document}".len()..];
    (&src[..ini], corpo.rfind("\\end{document}").map_or(corpo, |f| &corpo[..f]))
}

/// O argumento `{…}` do primeiro `comando` encontrado, já como texto puro.
fn argumento_de(src: &str, comando: &str, cores: &HashMap<String, u32>) -> Option<String> {
    Some(texto_puro(&inline(fonte_do_argumento(src, comando)?, cores)).trim().to_string())
}

/// O `{…}` do primeiro `comando` inteiro (o `\title`, não o `\titleformat`), como está no código.
fn fonte_do_argumento<'a>(src: &'a str, comando: &str) -> Option<&'a str> {
    let mut i = 0;
    while let Some(p) = src[i..].find(comando) {
        i += p + comando.len();
        if !src[i..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            return grupo(&src[i..]).map(|(g, _)| g);
        }
    }
    None
}

/// `{…}` no começo de `s` (espaços antes são aceitos): o interior e quantos bytes consumiu.
fn grupo(s: &str) -> Option<(&str, usize)> {
    delimitado(s, '{', '}')
}

/// `[…]` opcional.
fn opcional(s: &str) -> Option<(&str, usize)> {
    delimitado(s, '[', ']')
}

fn delimitado(s: &str, abre: char, fecha: char) -> Option<(&str, usize)> {
    let inicio = s.len() - s.trim_start().len();
    let resto = &s[inicio..];
    if !resto.starts_with(abre) {
        return None;
    }
    let mut prof = 0;
    let mut escapou = false;
    for (i, c) in resto.char_indices() {
        if escapou {
            escapou = false;
        } else if c == '\\' {
            escapou = true;
        } else if c == abre {
            prof += 1;
        } else if c == fecha {
            prof -= 1;
            if prof == 0 {
                return Some((&resto[1..i], inicio + i + 1));
            }
        }
    }
    None
}

/// O nome do comando que começa em `s` (`s` começa com `\`) e o tamanho total, com a barra e o `*`.
fn comando(s: &str) -> (&str, usize) {
    let resto = &s[1..];
    let letras = resto.chars().take_while(char::is_ascii_alphabetic).count();
    if letras == 0 {
        let n = resto.chars().next().map_or(0, char::len_utf8);
        return (&resto[..n], 1 + n);
    }
    let estrela = usize::from(resto[letras..].starts_with('*'));
    (&resto[..letras], 1 + letras + estrela)
}

fn parsear_cor(modelo: &str, espec: &str) -> Option<u32> {
    let mod_limpo = modelo.trim().to_ascii_uppercase();
    let esp = espec.trim();
    if mod_limpo == "HTML" || mod_limpo == "HEX" {
        let hex = esp.trim_start_matches('#');
        if hex.len() == 6 {
            let v = u32::from_str_radix(hex, 16).ok()?;
            return Some((v << 8) | 0xFF);
        }
    } else if modelo.trim() == "rgb" {
        // `rgb` (minúsculo) do `xcolor` vai de 0 a 1; `RGB`, de 0 a 255.
        let partes: Vec<f32> = esp.split(',').filter_map(|p| p.trim().parse::<f32>().ok()).collect();
        if let [r, g, b] = partes[..] {
            let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
            return Some((c(r) << 24) | (c(g) << 16) | (c(b) << 8) | 0xFF);
        }
    } else if mod_limpo == "RGB" {
        let partes: Vec<&str> = esp.split(',').collect();
        if partes.len() == 3 {
            let r = partes[0].trim().parse::<u8>().ok()?;
            let g = partes[1].trim().parse::<u8>().ok()?;
            let b = partes[2].trim().parse::<u8>().ok()?;
            return Some(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | 0xFF);
        }
    } else if mod_limpo == "GRAY" {
        let g = esp.parse::<f32>().ok()?;
        let v = (g.clamp(0.0, 1.0) * 255.0) as u8;
        return Some(((v as u32) << 24) | ((v as u32) << 16) | ((v as u32) << 8) | 0xFF);
    }
    None
}

fn resolver_cor(cores: &HashMap<String, u32>, nome: &str) -> Option<u32> {
    let limpo = nome.trim();
    // `xcolor`: `azul!80` = 80% azul e 20% branco; `azul!30!preto` = 30% azul e 70% preto.
    if let Some((a, resto)) = limpo.split_once('!') {
        let (p, b) = resto.split_once('!').unwrap_or((resto, "white"));
        let (ca, cb, p) = (resolver_cor(cores, a)?, resolver_cor(cores, b)?, p.trim().parse::<f32>().ok()?.clamp(0.0, 100.0) / 100.0);
        let canal = |c: u32, k: u32| ((c >> k) & 0xFF) as f32;
        let mistura = |k: u32| ((canal(ca, k) * p + canal(cb, k) * (1.0 - p)).round() as u32) << k;
        return Some(mistura(24) | mistura(16) | mistura(8) | 0xFF);
    }
    if let Some(&c) = cores.get(limpo) {
        return Some(c);
    }
    match limpo.to_ascii_lowercase().as_str() {
        "white" | "branco" => Some(0xFFFFFFFF),
        "black" | "preto" => Some(0x000000FF),
        "red" | "vermelho" => Some(0xE03030FF),
        "green" | "verde" => Some(0x20A040FF),
        "blue" | "azul" => Some(0x2060D0FF),
        "magenta" => Some(0xFF00FFFF),
        "cyan" => Some(0x00FFFFFF),
        // Do `dvipsnames` (CMYK → RGB como o `xcolor` converte), os que aparecem em `hypersetup`.
        "navyblue" => Some(0x0F75FFFF),
        "royalblue" => Some(0x0080FFFF),
        "midnightblue" => Some(0x00708FFF),
        "forestgreen" => Some(0x00E000FF),
        "olivegreen" => Some(0x009900FF),
        "maroon" => Some(0xAD0000FF),
        "brickred" => Some(0xB80000FF),
        "darkblue" => Some(0x1A3A5CFF),
        "midblue" => Some(0x2E6DA4FF),
        "lightblue" => Some(0xD6E8F7FF),
        "lightgray" | "lightgrey" => Some(0xF4F6F9FF),
        "gray" | "grey" => Some(0x808080FF),
        "graytext" => Some(0x555555FF),
        "darktext" => Some(0x1C1C1CFF),
        _ => {
            let hex = limpo.trim_start_matches('#');
            if hex.len() == 6 {
                if let Ok(v) = u32::from_str_radix(hex, 16) {
                    return Some((v << 8) | 0xFF);
                }
            }
            None
        }
    }
}

fn extrair_definicoes(src: &str) -> (HashMap<String, u32>, HashMap<String, MacroDef>) {
    let mut cores = HashMap::new();
    let mut macros = HashMap::new();
    let mut i = 0;
    while i < src.len() {
        let r = &src[i..];
        if r.starts_with("\\definecolor") {
            let mut p = 12;
            if let Some((nome, k1)) = grupo(&r[p..]) {
                p += k1;
                if let Some((modelo, k2)) = grupo(&r[p..]) {
                    p += k2;
                    if let Some((espec, k3)) = grupo(&r[p..]) {
                        if let Some(c) = parsear_cor(modelo, espec) {
                            cores.insert(nome.trim().to_string(), c);
                        }
                        i += p + k3;
                        continue;
                    }
                }
            }
        } else if let Some((nome, args, corpo, k)) = definicao_tex(r) {
            macros.insert(nome.to_string(), MacroDef { args, corpo: corpo.to_string(), ..MacroDef::default() });
            i += k;
            continue;
        } else if let Some(resto) = r.strip_prefix("\\newcounter") {
            // O contador e o pai que o zera (`\newcounter{caso}[section]`), para a expansão.
            if let Some((nome, k)) = grupo(resto) {
                let (pai, o) = opcional(&resto[k..]).unwrap_or(("", 0));
                macros.insert(format!("@contador:{}", nome.trim()), MacroDef { corpo: pai.trim().to_string(), ..MacroDef::default() });
                i += 11 + k + o;
                continue;
            }
            i += 11;
            continue;
        } else if let Some((nome, def, k)) = definicao_tcbox(r) {
            macros.insert(nome, def);
            i += k;
            continue;
        } else if let Some(campo) = ["\\title", "\\author", "\\date"].into_iter().find(|c| r.starts_with(c) && !r[c.len()..].starts_with(char::is_alphabetic)) {
            // O `\@title` que a classe usa no `\maketitle` dela.
            if let Some((g, k)) = grupo(&r[campo.len()..]) {
                macros.insert(format!("Arroba{}", &campo[1..]), MacroDef { corpo: g.to_string(), ..MacroDef::default() });
                i += campo.len() + k;
                continue;
            }
        } else if let Some((chave, def, k)) = definicao_latex(r) {
            // As definições de dentro (o `\newcommand{\atorPrimario}` de um ambiente) valem também.
            for corpo in [Some(&def.corpo), def.fim.as_ref()].into_iter().flatten() {
                let (c, m) = extrair_definicoes(&corpo.replace("##", "#"));
                cores.extend(c);
                for (n, d) in m {
                    macros.entry(n).or_insert(d);
                }
            }
            macros.insert(chave, def);
            i += k;
            continue;
        } else if r.starts_with('\\') {
            // Um "setter" usado no preâmbulo (`\subtitle{…}` = `\gdef\@subtitle{#1}`): a definição vale já.
            let (nome, n) = comando(r);
            let setter = macros.get(nome).filter(|m| m.args > 0 && m.fim.is_none() && m.corpo.trim_start().starts_with(['\\']) && definicao_tex(m.corpo.trim_start()).is_some());
            if let Some((corpo, k)) = setter.and_then(|m| argumentos_do_uso(m, &r[n..]).map(|(a, k)| (substituir_argumentos(&m.corpo, &a), k))) {
                if let Some((alvo, args, def, _)) = definicao_tex(corpo.trim_start()) {
                    macros.insert(alvo.to_string(), MacroDef { args, corpo: def.to_string(), ..MacroDef::default() });
                }
                i += n + k;
                continue;
            }
        }
        let c = r.chars().next().unwrap_or(' ');
        i += c.len_utf8();
    }
    (cores, macros)
}

/// `\newtcbox{\nome}[n][padrão]{opções}` do `tcolorbox`: a caixa em linha vira um `\colorbox` com o
/// `colback` (os `n` argumentos ficam nas opções; o texto é o último).
fn definicao_tcbox(r: &str) -> Option<(String, MacroDef, usize)> {
    let mut p = "\\newtcbox".len();
    if !r.starts_with("\\newtcbox") {
        return None;
    }
    let (nome, k) = grupo(&r[p..])?;
    p += k;
    let mut args = 0;
    if let Some((o, k)) = opcional(&r[p..]) {
        args = o.trim().parse::<usize>().unwrap_or(0);
        p += k;
    }
    let mut padroes = vec![None; args + 1];
    if let Some((o, k)) = opcional(&r[p..]) {
        padroes[0] = Some(o.to_string());
        p += k;
    }
    let (opcoes, k) = grupo(&r[p..])?;
    p += k;
    let fundo = opcoes_chave_valor(opcoes).into_iter().find(|(c, _)| c == "colback").map_or("white".to_string(), |(_, v)| v);
    if padroes.iter().all(Option::is_none) {
        padroes.clear();
    }
    let corpo = format!("\\colorbox{{{fundo}}}{{#{}}}", args + 1);
    Some((nome.trim().trim_start_matches('\\').to_string(), MacroDef { args: args + 1, corpo, padroes, fim: None }, p))
}

/// `\def\nome#1#2{corpo}` (e `\let\a\b`, que vira `\a` = `\b`): o nome, quantos argumentos,
/// o corpo e quanto foi lido.
fn definicao_tex(r: &str) -> Option<(&str, usize, &str, usize)> {
    if !r.starts_with('\\') {
        return None;
    }
    let (cmd, n) = comando(r);
    if !matches!(cmd, "def" | "gdef" | "edef" | "let") {
        return None;
    }
    let depois = r[n..].trim_start();
    let mut k = r.len() - depois.len();
    if !depois.starts_with('\\') {
        return None;
    }
    let (nome, m) = comando(depois);
    k += m;
    if cmd == "let" {
        let alvo = r[k..].trim_start().trim_start_matches('=').trim_start();
        k = r.len() - alvo.len();
        if !alvo.starts_with('\\') {
            return None;
        }
        let (_, a) = comando(alvo);
        return Some((nome, 0, &alvo[..a], k + a));
    }
    let parametros = &r[k..r[k..].find('{').map_or(k, |p| k + p)];
    if !parametros.chars().all(|c| c == '#' || c.is_ascii_digit() || c.is_whitespace()) {
        return None;
    }
    k += parametros.len();
    let (corpo, g) = grupo(&r[k..])?;
    Some((nome, parametros.matches('#').count(), corpo, k + g))
}

fn expandir_macros(mut texto: String, macros: &HashMap<String, MacroDef>) -> String {
    // As definições feitas no texto (`\def` de um "setter" como o `\disposicao{…}`) valem dali em diante.
    let mut macros = macros.clone();
    macros.entry("Arrobaempty".into()).or_default();
    macros.entry("Arrobadate".into()).or_insert_with(|| MacroDef { corpo: "\\today".into(), ..MacroDef::default() });
    macros.entry("mdfsubtitle".into()).or_insert_with(|| MacroDef { args: 1, corpo: "\\par\\textbf{#1}\\par ".into(), ..MacroDef::default() });
    let mut contadores: HashMap<String, u32> = HashMap::new();
    // As listas do `etoolbox` (`\eappto\lista{…}`) mudam ao longo do texto: só são abertas na
    // passada que decide, na ordem em que aparecem.
    let mut variaveis: std::collections::HashSet<String> = alvos_de_appto(&texto);
    for m in macros.values() {
        variaveis.extend(alvos_de_appto(&m.corpo));
    }
    // O `\ifx` só é decidido quando o texto parou de mudar: antes disso um `\def` ainda pode aparecer.
    let mut decidir = false;
    for _ in 0..20 {
        let mut mudou = false;
        let mut saida = String::with_capacity(texto.len());
        let mut i = 0;
        // As divisões são contadas de novo a cada passada, na ordem do texto.
        for d in ["chapter", "section", "subsection"] {
            contadores.insert(d.into(), 0);
        }
        while i < texto.len() {
            let resto = &texto[i..];
            if resto.starts_with('\\') {
                if let Some((nome, args, corpo, k)) = definicao_tex(resto) {
                    macros.insert(nome.to_string(), MacroDef { args, corpo: corpo.to_string(), ..MacroDef::default() });
                    i += k;
                    continue;
                }
                if let Some((chave, def, k)) = definicao_latex(resto) {
                    macros.insert(chave, def);
                    i += k;
                    continue;
                }
                let (nome, n) = comando(resto);
                if matches!(nome, "chapter" | "section" | "subsection") && !texto[i + n..].starts_with('*') {
                    passar_contador(&mut contadores, &macros, nome);
                }
                if let Some(passo) = contador(nome, &texto[i + n..], &mut contadores, &macros).or_else(|| etoolbox(nome, &texto[i + n..], &mut macros, decidir)) {
                    match passo {
                        Passo::Pular(k) => i += n + k,
                        Passo::Copiar(k) => {
                            saida.push_str(&texto[i..i + n + k]);
                            i += n + k;
                            continue;
                        }
                        // O que saiu é lido de novo já nesta passada, na ordem (o TeX também faz assim).
                        Passo::Trocar(novo, k) => texto = format!("{}{novo}{}", &texto[..i], &texto[i + n + k..]),
                    }
                    mudou = true;
                    continue;
                }
                if variaveis.contains(nome) && !decidir {
                    saida.push_str(&texto[i..i + n]);
                    i += n;
                    continue;
                }
                if nome == "ifx" {
                    if decidir {
                        if let Some((ramo, k)) = decidir_ifx(&texto[i + n..], &macros) {
                            saida.push_str(&ramo);
                            i += n + k;
                            mudou = true;
                            continue;
                        }
                    }
                    // O `\ifx` compara as duas macros sem abri-las: elas passam como estão.
                    let mut k = i + n;
                    for _ in 0..2 {
                        let t = &texto[k..];
                        let espacos = t.len() - t.trim_start().len();
                        let t = t.trim_start();
                        k += espacos + if t.starts_with('\\') { comando(t).1 } else { t.chars().next().map_or(0, char::len_utf8) };
                    }
                    saida.push_str(&texto[i..k]);
                    i = k;
                    continue;
                }
                // `\begin{x}`/`\end{x}` de um ambiente do `\newenvironment`: o código de abrir (com os
                // argumentos) e o de fechar.
                if matches!(nome, "begin" | "end") {
                    if let Some((amb, g)) = grupo(&texto[i + n..]) {
                        if let Some(m) = macros.get(&chave_de_ambiente(amb.trim())) {
                            if nome == "end" {
                                saida.push_str(&substituir_argumentos(m.fim.as_deref().unwrap_or(""), &[]));
                                i += n + g;
                                mudou = true;
                                continue;
                            }
                            if let Some((args, k)) = argumentos_do_uso(m, &texto[i + n + g..]) {
                                colar(&mut saida, &substituir_argumentos(&m.corpo, &args));
                                i += n + g + k;
                                mudou = true;
                                continue;
                            }
                        }
                    }
                }
                if let Some((args, k)) = macros.get(nome).filter(|m| m.fim.is_none()).and_then(|m| argumentos_do_uso(m, &texto[i + n..]).map(|a| (m, a))).map(|(m, (args, k))| (substituir_argumentos(&m.corpo, &args), k)) {
                    mudou = true;
                    // Com contador dentro, o valor tem de sair agora, antes do próximo `\refstepcounter`.
                    if ["stepcounter", "\\arabic{", "\\roman{", "\\Roman{", "\\alph{", "\\Alph{", "\\the"].iter().any(|c| args.contains(c)) {
                        texto = format!("{}{args}{}", &texto[..i], &texto[i + n + k..]);
                        continue;
                    }
                    colar(&mut saida, &args);
                    i += n + k;
                    continue;
                }
            }
            let c = resto.chars().next().unwrap_or(' ');
            saida.push(c);
            i += c.len_utf8();
        }
        texto = saida;
        match (mudou, decidir) {
            (false, true) => break,
            (false, false) => decidir = true,
            (true, _) => decidir = false,
        }
    }
    texto
}

/// O código com cada tabulação trocada por espaços até a próxima parada (a cada `passo` colunas).
fn sem_tabulacao(codigo: &str, passo: usize) -> String {
    if !codigo.contains('\t') {
        return codigo.to_string();
    }
    let passo = if passo == 0 { 8 } else { passo };
    let mut saida = String::with_capacity(codigo.len());
    let mut coluna = 0;
    for c in codigo.chars() {
        match c {
            '\t' => {
                let n = passo - coluna % passo;
                saida.extend(std::iter::repeat_n(' ', n));
                coluna += n;
            }
            '\n' => {
                saida.push(c);
                coluna = 0;
            }
            _ => {
                saida.push(c);
                coluna += 1;
            }
        }
    }
    saida
}

/// `comando` aparece em `texto` fora de qualquer `{…}`.
fn fora_de_grupo(texto: &str, comando: &str) -> bool {
    let mut prof = 0i32;
    let mut escapado = false;
    for (i, c) in texto.char_indices() {
        // `\{` e `\}` são as chaves escritas (o `literate=` do `listings`), não grupo.
        if std::mem::take(&mut escapado) {
            continue;
        }
        match c {
            '{' => prof += 1,
            '}' => prof -= 1,
            '\\' if prof == 0 && texto[i..].starts_with(comando) && !texto[i + comando.len()..].starts_with(char::is_alphabetic) => return true,
            '\\' => escapado = true,
            _ => {}
        }
    }
    false
}

/// O que fazer com um comando lido na expansão.
enum Passo {
    /// Some com `k` bytes depois do comando.
    Pular(usize),
    /// Passa como está (o comando e `k` bytes).
    Copiar(usize),
    /// Vira o texto dado, lido de novo.
    Trocar(String, usize),
}

/// As listas que o `\appto`/`\eappto` do `etoolbox` vão enchendo.
fn alvos_de_appto(s: &str) -> std::collections::HashSet<String> {
    let mut alvos = std::collections::HashSet::new();
    for (p, _) in s.match_indices("appto") {
        let depois = s[p + 5..].trim_start();
        if s[..p].ends_with(['\\', 'e', 'g', 'x']) && depois.starts_with('\\') {
            alvos.insert(comando(depois).0.to_string());
        }
    }
    alvos
}

/// Um contador que andou zera os filhos (`\newcounter{caso}[section]` e as divisões abaixo).
fn passar_contador(contadores: &mut HashMap<String, u32>, macros: &HashMap<String, MacroDef>, nome: &str) {
    *contadores.entry(nome.to_string()).or_default() += 1;
    let filhos: Vec<String> = macros.iter().filter(|(k, m)| k.starts_with("@contador:") && m.corpo == nome).map(|(k, _)| k[10..].to_string()).collect();
    for f in filhos {
        contadores.insert(f, 0);
    }
    let abaixo: &[&str] = match nome {
        "chapter" => &["section", "subsection"],
        "section" => &["subsection"],
        _ => &[],
    };
    for f in abaixo {
        contadores.insert(f.to_string(), 0);
    }
}

/// `\refstepcounter{c}`, `\arabic{c}` (e `\roman`, `\alph`…) e `\thec` de um contador do texto.
fn contador(nome: &str, depois: &str, contadores: &mut HashMap<String, u32>, macros: &HashMap<String, MacroDef>) -> Option<Passo> {
    let existe = |c: &str| contadores.contains_key(c) || macros.contains_key(&format!("@contador:{c}"));
    match nome {
        "refstepcounter" | "stepcounter" => {
            let (c, k) = grupo(depois)?;
            existe(c.trim()).then_some(())?;
            passar_contador(contadores, macros, c.trim());
            Some(Passo::Pular(k))
        }
        "arabic" | "roman" | "Roman" | "alph" | "Alph" => {
            let (c, k) = grupo(depois)?;
            existe(c.trim()).then_some(())?;
            let marca = match nome {
                "arabic" => "#1",
                "roman" => "#i",
                "Roman" => "#I",
                "alph" => "#a",
                _ => "#A",
            };
            let v = contadores.get(c.trim()).copied().unwrap_or(0);
            Some(Passo::Trocar(featherdocs_core::paginacao::rotulo_do_modelo(&format!("|{marca}"), v)?, k))
        }
        _ => {
            let c = nome.strip_prefix("the")?;
            if macros.contains_key(nome) || !existe(c) {
                return None;
            }
            // `\thesubsection` = "seção.subseção", como no `article`.
            let v = match c {
                "subsection" => format!("{}.{}", contadores.get("section").copied().unwrap_or(0), contadores.get(c).copied().unwrap_or(0)),
                _ => contadores.get(c).copied().unwrap_or(0).to_string(),
            };
            let espaco = if depois.starts_with(char::is_alphabetic) { " " } else { "" };
            Some(Passo::Trocar(format!("{v}{espaco}"), 0))
        }
    }
}

/// Os registros do TeX que um pacote muda com `\parskip=…`; o FeatherDocs tem os próprios espaços.
const REGISTROS: &[&str] = &[
    "hbadness", "vbadness", "hfuzz", "vfuzz", "overfullrule", "parskip", "parindent", "baselineskip", "tolerance", "pretolerance", "emergencystretch",
    "clubpenalty", "widowpenalty", "displaywidowpenalty", "interlinepenalty", "brokenpenalty", "hyphenpenalty", "exhyphenpenalty", "linepenalty", "lineskip",
    "lineskiplimit", "topskip", "footskip", "headheight", "headsep", "abovedisplayskip", "belowdisplayskip", "columnsep", "marginparwidth",
];

/// O valor de uma atribuição do TeX (`=0.3\baselineskip`, `by 0pt plus 2pt`, `=10000`): quantos bytes.
fn valor_tex(s: &str) -> Option<usize> {
    let pular = |s: &str, k: usize| k + s[k..].len() - s[k..].trim_start().len();
    let mut k = pular(s, 0);
    for prefixo in ["=", "by "] {
        if s[k..].starts_with(prefixo) {
            k = pular(s, k + prefixo.len());
        }
    }
    let dimensao = |k: usize| -> Option<usize> {
        let t = &s[k..];
        let num = t.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+'))).unwrap_or(t.len());
        let resto = &t[num..];
        if resto.starts_with('\\') {
            return Some(k + num + comando(resto).1);
        }
        if num == 0 {
            return None;
        }
        let unidade = resto.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(resto.len());
        let unidade = if matches!(&resto[..unidade], "pt" | "mm" | "cm" | "in" | "em" | "ex" | "bp" | "pc" | "sp" | "fil" | "fill" | "filll") { unidade } else { 0 };
        Some(k + num + unidade)
    };
    k = dimensao(k)?;
    for extra in ["plus", "minus"] {
        let j = pular(s, k);
        if s[j..].starts_with(extra) {
            if let Some(f) = dimensao(pular(s, j + extra.len())) {
                k = f;
            }
        }
    }
    Some(k)
}

/// O texto de uma condição do `ifthen` com as macros sem argumento abertas (`\listadepresentes`).
fn valor_de(s: &str, macros: &HashMap<String, MacroDef>) -> String {
    let mut v = s.to_string();
    for _ in 0..10 {
        let mut novo = String::with_capacity(v.len());
        let mut i = 0;
        while i < v.len() {
            let r = &v[i..];
            if r.starts_with('\\') {
                let (nome, n) = comando(r);
                if let Some(m) = macros.get(nome).filter(|m| m.args == 0 && m.fim.is_none()) {
                    novo.push_str(&m.corpo);
                    i += n;
                    continue;
                }
            }
            let c = r.chars().next().unwrap_or(' ');
            novo.push(c);
            i += c.len_utf8();
        }
        if novo == v {
            break;
        }
        v = novo;
    }
    v.trim().to_string()
}

/// `\equal{a}{b}`, `\isempty{x}` e o `\NOT` do `ifthen`; `None` se a condição não é dessas.
fn condicao(c: &str, macros: &HashMap<String, MacroDef>) -> Option<bool> {
    let c = c.trim();
    if let Some(resto) = c.strip_prefix("\\NOT") {
        return condicao(resto, macros).map(|v| !v);
    }
    let (nome, n) = comando(c);
    let depois = &c[n..];
    match nome {
        "equal" => {
            let (a, k) = grupo(depois)?;
            let (b, j) = grupo(&depois[k..])?;
            depois[k + j..].trim().is_empty().then(|| valor_de(a, macros) == valor_de(b, macros))
        }
        "isempty" => {
            let (a, k) = grupo(depois)?;
            depois[k..].trim().is_empty().then(|| valor_de(a, macros).is_empty())
        }
        _ => None,
    }
}

/// `n` grupos `{…}` seguidos.
fn grupos(s: &str, n: usize) -> Option<(Vec<&str>, usize)> {
    let mut k = 0;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let (g, j) = grupo(&s[k..])?;
        v.push(g);
        k += j;
    }
    Some((v, k))
}

/// Texto de enchimento do `\lipsum`: o parágrafo `n` tem o tamanho (em letras) do `n` do pacote, para
/// a página quebrar no mesmo lugar; as palavras são as nossas, a partir de um ponto que muda com `n`.
fn lipsum(n: usize) -> String {
    const PARAGRAFOS: [&str; 5] = [
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer posuere erat a ante venenatis dapibus, sed vehicula nisi tincidunt. Curabitur blandit tempus porttitor, nulla vitae elit libero, a pharetra augue. Donec ullamcorper nulla non metus auctor fringilla.",
        "Nulla facilisi. Aenean lacinia bibendum nulla sed consectetur. Praesent commodo cursus magna, vel scelerisque nisl consectetur et. Maecenas faucibus mollis interdum, cras mattis consectetur purus sit amet fermentum. Etiam porta sem malesuada magna mollis euismod.",
        "Vestibulum id ligula porta felis euismod semper. Morbi leo risus, porta ac consectetur ac, vestibulum at eros. Fusce dapibus, tellus ac cursus commodo, tortor mauris condimentum nibh, ut fermentum massa justo sit amet risus. Sed posuere consectetur est at lobortis.",
        "Cum sociis natoque penatibus et magnis dis parturient montes, nascetur ridiculus mus. Duis mollis, est non commodo luctus, nisi erat porttitor ligula, eget lacinia odio sem nec elit. Nullam quis risus eget urna mollis ornare vel eu leo.",
        "Pellentesque ornare sem lacinia quam venenatis vestibulum. Vivamus sagittis lacus vel augue laoreet rutrum faucibus dolor auctor. Integer posuere erat a ante venenatis dapibus posuere velit aliquet. Donec id elit non mi porta gravida at eget metus.",
    ];
    // O tamanho de cada um dos 150 parágrafos do `lipsum.ltd.tex` (só a medida, não o texto).
    const TAMANHOS: [usize; 150] = [
        867, 550, 711, 488, 711, 552, 634, 646, 867, 564, 397, 573, 412, 820, 614, 629, 511, 712, 490, 570, 683, 594, 453, 544, 734, 554, 471, 487,
        856, 673, 602, 549, 614, 442, 650, 770, 858, 634, 439, 787, 637, 710, 430, 1058, 928, 566, 360, 558, 447, 527, 600, 567, 758, 525, 576, 425,
        398, 579, 759, 802, 611, 513, 580, 625, 355, 313, 723, 405, 523, 832, 719, 619, 589, 555, 305, 825, 857, 408, 486, 1041, 621, 649, 487, 599,
        571, 787, 543, 508, 518, 948, 621, 449, 695, 963, 535, 911, 777, 884, 1040, 576, 313, 550, 344, 568, 580, 853, 603, 446, 877, 462, 563, 745,
        658, 376, 380, 607, 501, 559, 586, 889, 335, 546, 662, 770, 931, 757, 496, 439, 613, 398, 481, 495, 324, 520, 708, 508, 450, 458, 663, 443,
        567, 647, 478, 393, 575, 643, 904, 556, 597, 491
    ];
    let palavras: Vec<&str> = PARAGRAFOS.iter().flat_map(|p| p.split(' ')).collect();
    let i = n.saturating_sub(1) % TAMANHOS.len();
    let mut texto = String::new();
    for w in palavras.iter().cycle().skip(i * 37 % palavras.len()) {
        if !texto.is_empty() && texto.len() + 1 + w.len() > TAMANHOS[i] {
            break;
        }
        if !texto.is_empty() {
            texto.push(' ');
        }
        texto.push_str(w);
    }
    let texto = texto.trim_end_matches([',', '.']);
    let mut c = texto.chars();
    c.next().map_or(String::new(), |p| format!("{}{}.", p.to_uppercase(), c.as_str()))
}

/// Os comandos do `etoolbox`/`ifthen`/`lipsum`, as atribuições do TeX e os erros de pacote, que mudam
/// o texto antes da leitura. `depois` é o que vem logo depois do comando.
fn etoolbox(nome: &str, depois: &str, macros: &mut HashMap<String, MacroDef>, decidir: bool) -> Option<Passo> {
    let alvo = |t: &str| -> Option<(String, usize)> {
        let s = t.trim_start();
        let espacos = t.len() - s.len();
        if s.starts_with('\\') {
            let (n, k) = comando(s);
            return Some((n.to_string(), espacos + k));
        }
        let (g, k) = grupo(t)?;
        Some((g.trim().trim_start_matches('\\').to_string(), k))
    };
    Some(match nome {
        "PackageError" | "ClassError" | "GenericError" => Passo::Pular(grupos(depois, 3)?.1),
        "PackageWarning" | "ClassWarning" | "PackageInfo" | "ClassInfo" | "PackageWarningNoLine" => Passo::Pular(grupos(depois, 2)?.1),
        // O `\date{…}` redefine o `\@date` dali em diante (a classe que fez `\def\@date{\today}` antes perde).
        "date" | "title" | "author" => {
            let (g, k) = grupo(depois)?;
            macros.insert(format!("Arroba{nome}"), MacroDef { corpo: g.to_string(), ..MacroDef::default() });
            Passo::Pular(k)
        }
        "advance" => {
            let (_, k) = alvo(depois)?;
            Passo::Pular(k + valor_tex(&depois[k..])?)
        }
        _ if REGISTROS.contains(&nome) && depois.trim_start().starts_with(|c: char| c == '=' || c.is_ascii_digit()) => Passo::Pular(valor_tex(depois)?),
        "lipsum" => {
            let (faixa, k) = opcional(depois).unwrap_or(("1-7", 0));
            let (a, b) = faixa.split_once('-').unwrap_or((faixa, faixa));
            let a: usize = a.trim().parse().unwrap_or(1);
            let b: usize = b.trim().parse().unwrap_or(a).clamp(a, a + 50);
            let texto: Vec<String> = (a..=b).map(lipsum).collect();
            Passo::Trocar(format!("{}\\par ", texto.join("\\par ")), k)
        }
        "csuse" => {
            let (g, k) = grupo(depois)?;
            let espaco = if depois[k..].starts_with(char::is_alphabetic) { " " } else { "" };
            Passo::Trocar(format!("\\{}{espaco}", g.trim()), k)
        }
        "csdef" | "csgdef" | "csedef" | "csxdef" => {
            let (v, k) = grupos(depois, 2)?;
            macros.insert(v[0].trim().to_string(), MacroDef { corpo: v[1].to_string(), ..MacroDef::default() });
            Passo::Pular(k)
        }
        "appto" | "eappto" | "gappto" | "xappto" => {
            let (n, k) = alvo(depois)?;
            let (g, j) = grupo(&depois[k..])?;
            macros.entry(n).or_default().corpo.push_str(g);
            Passo::Pular(k + j)
        }
        "ifdefempty" | "ifcsempty" | "ifdefvoid" | "ifcsvoid" => {
            let (n, k) = alvo(depois)?;
            let (ramos, j) = grupos(&depois[k..], 2)?;
            if !decidir {
                return Some(Passo::Copiar(k + j));
            }
            let vazio = macros.get(&n).is_none_or(|m| m.corpo.trim().is_empty());
            Passo::Trocar(ramos[if vazio { 0 } else { 1 }].to_string(), k + j)
        }
        "ifthenelse" => {
            let (v, k) = grupos(depois, 3)?;
            if !decidir {
                return Some(Passo::Copiar(k));
            }
            // A condição que não sabemos decidir fica como está (e vai para o aviso).
            let Some(sim) = condicao(v[0], macros) else { return Some(Passo::Copiar(k)) };
            Passo::Trocar(v[if sim { 1 } else { 2 }].to_string(), k)
        }
        _ => return None,
    })
}

/// Junta `texto` ao fim de `saida`: se ela acaba num comando (`\noindent`) e o texto começa com letra,
/// um espaço os mantém separados (no TeX já eram dois tokens).
fn colar(saida: &mut String, texto: &str) {
    let antes = saida.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    if antes.ends_with('\\') && antes.len() + 1 < saida.len() && texto.starts_with(|c: char| c.is_alphabetic()) {
        saida.push(' ');
    }
    saida.push_str(texto);
}

/// `\ifx\a\b sim\else não\fi` (o texto começa logo depois do `\ifx`): iguais se as duas macros, abertas
/// até o fim, dão o mesmo texto. Devolve o ramo e quanto foi lido; `None` se não achou o `\fi`.
fn decidir_ifx(texto: &str, macros: &HashMap<String, MacroDef>) -> Option<(String, usize)> {
    let token = |t: &str| -> Option<(String, usize)> {
        let espacos = t.len() - t.trim_start().len();
        let t = t.trim_start();
        if t.starts_with('\\') {
            let (nome, n) = comando(t);
            let mut valor = format!("\\{nome}");
            // Abre as macros sem argumento (o `\let` vira um apelido) até chegar no texto.
            for _ in 0..10 {
                let Some(m) = valor.strip_prefix('\\').filter(|v| v.chars().all(|c| c.is_ascii_alphabetic())).and_then(|v| macros.get(v)).filter(|m| m.args == 0 && m.fim.is_none()) else { break };
                valor = m.corpo.trim().to_string();
            }
            Some((valor, espacos + n))
        } else {
            let c = t.chars().next()?;
            Some((c.to_string(), espacos + c.len_utf8()))
        }
    };
    let (a, ka) = token(texto)?;
    let (b, kb) = token(&texto[ka..])?;
    let inicio = ka + kb;
    // O `\else` e o `\fi` deste `\if`, pulando os de dentro.
    let (mut prof, mut j, mut senao) = (0, inicio, None);
    while j < texto.len() {
        if texto[j..].starts_with('\\') {
            let (n, k) = comando(&texto[j..]);
            if n.starts_with("if") {
                prof += 1;
            } else if n == "fi" {
                if prof == 0 {
                    let ramo = if a == b { &texto[inicio..senao.unwrap_or(j)] } else { senao.map_or("", |e| &texto[e + 5..j]) };
                    return Some((ramo.to_string(), j + 3));
                }
                prof -= 1;
            } else if n == "else" && prof == 0 {
                senao = Some(j);
            }
            j += k;
            continue;
        }
        j += texto[j..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

// ---------------------------------------------------------------------------
// Texto em linha
// ---------------------------------------------------------------------------

/// O estado das declarações em vigor (`\bfseries`, `\small`, `\color{…}`): vale até o fim do grupo
/// `{…}` ou do ambiente em que foi feita, e atravessa parágrafos, como no TeX.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Estado {
    estilo: u8,
    cor: u32,
    /// Marca-texto do `\hl` e a cor do `\sethlcolor` em vigor (0 = o amarelo do `soul`).
    realce: u32,
    cor_do_realce: u32,
    /// `\textsuperscript` = 1, `\textsubscript` = -1.
    posicao: i8,
    /// Corpo em pontos; 0 = o normal do documento.
    tamanho: f32,
    /// O fator do `\scalebox` em volta (0 = 1); vale para os desenhos do `\tikz`.
    escala: f32,
    /// `\sffamily`/`\textsf` = `Some(0)`, `\rmfamily`/`\textrm` = `Some(SERIFA)`; `None` = a da classe.
    familia: Option<u8>,
}

/// A classe do documento: o corpo (`10pt`, `11pt`, `12pt`) e a família padrão.
#[derive(Clone, Copy, Debug)]
struct Classe {
    /// Os corpos de `\tiny` a `\Huge` nesta classe.
    tamanhos: [f32; 10],
    /// O corpo do texto normal, quando a classe o diz (0 = o do FeatherDocs).
    corpo: f32,
    /// `SERIFA` se a família padrão é a romana (o padrão do LaTeX), 0 se é `\sfdefault`.
    familia: u8,
    /// O `scale=` da letra sem serifa (`\usepackage[scale=0.88]{tgheros}`): muda o desenho da letra, não
    /// a entrelinha.
    escala: f32,
}

impl Default for Classe {
    fn default() -> Self {
        Self { tamanhos: TAMANHOS_10PT, corpo: 0.0, familia: 0, escala: 1.0 }
    }
}

const TAMANHOS_10PT: [f32; 10] = [5.0, 7.0, 8.0, 9.0, 10.0, 12.0, 14.4, 17.28, 20.74, 24.88];
const TAMANHOS_11PT: [f32; 10] = [6.0, 8.0, 9.0, 10.0, 10.95, 12.0, 14.4, 17.28, 20.74, 24.88];
const TAMANHOS_12PT: [f32; 10] = [6.0, 8.0, 10.0, 10.95, 12.0, 14.4, 17.28, 20.74, 24.88, 24.88];
const DECLARACOES_DE_TAMANHO: [&str; 10] = ["tiny", "scriptsize", "footnotesize", "small", "normalsize", "large", "Large", "LARGE", "huge", "Huge"];

impl Classe {
    /// A classe pelo `\documentclass[…]` e pelo preâmbulo (`\renewcommand{\familydefault}{\sfdefault}`).
    fn do_preambulo(preambulo: &str) -> Self {
        let opcoes = preambulo.find("\\documentclass").and_then(|i| opcional(&preambulo[i + 14..])).map_or("", |(o, _)| o);
        let tem = |o: &str| opcoes.split(',').any(|x| x.trim() == o);
        let (tamanhos, corpo) = if tem("12pt") {
            (TAMANHOS_12PT, 12.0)
        } else if tem("11pt") {
            (TAMANHOS_11PT, 0.0)
        } else if tem("10pt") {
            (TAMANHOS_10PT, 10.0)
        } else {
            (TAMANHOS_10PT, 0.0)
        };
        // `\sfdefault`, um `\sffamily` solto no preâmbulo (fora de grupo: o de um título não conta) ou o
        // `\setmainfont` de uma fonte sem serifa.
        let solto = preambulo.match_indices("\\sffamily").any(|(i, _)| preambulo[..i].matches('{').count() <= preambulo[..i].matches('}').count());
        let fonte_principal = fonte_do_argumento_opcional(preambulo, "\\setmainfont").unwrap_or_default().to_lowercase();
        let sem_serifa_principal = ["heros", "helvet", "arial", "inter", "roboto", "open sans", "lato", "source sans", "noto sans", "dejavu sans", "liberation sans", "fira sans", "montserrat"].iter().any(|f| fonte_principal.contains(f));
        let sem_serifa = preambulo.contains("\\sfdefault") || solto || sem_serifa_principal;
        let escala = ["{tgheros}", "{helvet}", "{heros}"]
            .iter()
            .filter_map(|p| preambulo.find(p))
            .find_map(|i| {
                let opcoes = &preambulo[preambulo[..i].rfind('[')? + 1..preambulo[..i].rfind(']')?];
                opcoes.split(',').find_map(|o| o.trim().strip_prefix("scaled=").or_else(|| o.trim().strip_prefix("scale="))?.trim().parse::<f32>().ok())
            })
            // No XeLaTeX/LuaLaTeX: o `Scale=` do `\setmainfont` (o mesmo que dá a fonte principal).
            .or_else(|| {
                let i = preambulo.rfind("\\setmainfont")?;
                let (o, _) = opcional(&preambulo[i + 12..])?;
                opcoes_chave_valor(o).into_iter().find(|(k, _)| k.trim() == "Scale").and_then(|(_, v)| v.trim().parse().ok())
            })
            .filter(|e| sem_serifa && (0.5..=1.5).contains(e))
            .unwrap_or(1.0);
        Self { tamanhos, corpo, familia: if sem_serifa { 0 } else { SERIFA }, escala }
    }

    /// A entrelinha do LaTeX nesta classe, na medida do FeatherDocs (1,15 = 1,4 × o corpo): o
    /// `\baselineskip` é 12 pt no corpo 10, 13,6 no 11 e 14,5 no 12.
    fn entrelinha(&self) -> f32 {
        let (linha, corpo) = match self.tamanhos[4] {
            c if c >= 12.0 => (14.5, 12.0),
            c if c > 10.0 => (13.6, 10.95),
            _ => (12.0, 10.0),
        };
        linha / corpo / 1.4 * 1.15 / self.escala
    }

    /// O `\partopsep` (2 pt no corpo 10, 3 no 11 e no 12): soma ao `\topsep` da lista que começa parágrafo.
    fn partopsep(&self) -> f32 {
        if self.tamanhos[4] > 10.0 { 3.0 } else { 2.0 }
    }

    /// O recuo do texto do item de `nivel` (0 = o de fora), da margem: o `\leftmargin` de cada nível
    /// somado (2,5em, 2,2em, 1,87em, 1,7em, 1em, 1em).
    fn recuo_da_lista(&self, nivel: u8) -> f32 {
        [2.5, 2.2, 1.87, 1.7, 1.0, 1.0].iter().take(usize::from(nivel.min(5)) + 1).sum::<f32>() * self.tamanhos[4]
    }

    /// O corpo de uma declaração de tamanho; `Some(0)` = o normal.
    fn tamanho_de(&self, nome: &str) -> Option<f32> {
        let i = DECLARACOES_DE_TAMANHO.iter().position(|d| *d == nome)?;
        Some(if i == 4 { 0.0 } else { self.tamanhos[i] })
    }
}

/// Texto em linha de LaTeX para trechos, sem contexto (título e autor do preâmbulo).
fn inline(src: &str, cores: &HashMap<String, u32>) -> Vec<Trecho> {
    let ctx = Ctx { cores: cores.clone(), ..Ctx::default() };
    let mut e = Estado::default();
    inline_em(src, &ctx, &mut e)
}

/// Texto em linha no contexto do documento: começa no estado `e` e o deixa como as declarações do texto o deixaram.
fn inline_em(src: &str, ctx: &Ctx, e: &mut Estado) -> Vec<Trecho> {
    let mut i = Inline { saida: Vec::new(), e: *e, link: None, ctx, aspas: 0 };
    i.corre(src);
    *e = i.e;
    aparar(i.saida)
}

/// Até onde vai o `\tikz` no meio do texto: o `{…}` dele ou o `;` fora de chaves (com ele).
fn fim_do_tikz(s: &str) -> usize {
    let mut k = opcional(s).map_or(0, |o| o.1);
    let resto = &s[k..];
    if let Some((_, g)) = grupo(resto).filter(|_| resto.trim_start().starts_with('{')) {
        return k + g;
    }
    let mut prof = 0;
    for (i, c) in resto.char_indices() {
        match c {
            '{' | '[' => prof += 1,
            '}' | ']' => prof -= 1,
            ';' if prof == 0 => return k + i + 1,
            _ => {}
        }
    }
    k += resto.len();
    k
}

/// Sem espaço nas pontas nem em volta de tabulação (`\hfill`).
fn aparar(mut saida: Vec<Trecho>) -> Vec<Trecho> {
    while saida.last().is_some_and(|t| t.texto.trim().is_empty() && !t.texto.contains('\t')) {
        saida.pop();
    }
    if let Some(p) = saida.first_mut() {
        // O espaço inquebrável (`~`, `\hspace`) fica: no LaTeX ele não some no começo da linha.
        p.texto = p.texto.trim_start_matches(|c: char| c.is_whitespace() && c != '\u{a0}').to_string();
    }
    if let Some(u) = saida.last_mut() {
        u.texto = u.texto.trim_end_matches([' ', featherdocs_core::modelo::QUEBRA_DE_LINHA]).to_string();
    }
    for k in 0..saida.len() {
        if let Some(t) = saida[k].texto.find('\t') {
            let (antes, depois) = saida[k].texto.split_at(t);
            saida[k].texto = format!("{}\t{}", antes.trim_end_matches(' '), depois[1..].trim_start_matches(' '));
            if saida[k].texto.starts_with('\t') && k > 0 {
                let a = saida[k - 1].texto.trim_end_matches(' ').len();
                saida[k - 1].texto.truncate(a);
            }
            if saida[k].texto.ends_with('\t') {
                if let Some(prox) = saida.get_mut(k + 1) {
                    prox.texto = prox.texto.trim_start_matches(' ').to_string();
                }
            }
        }
    }
    saida.retain(|t| !t.texto.is_empty());
    saida
}

struct Inline<'a> {
    saida: Vec<Trecho>,
    e: Estado,
    link: Option<String>,
    ctx: &'a Ctx,
    /// Quantos `\enquote` estão abertos (o de dentro usa as aspas simples).
    aspas: u8,
}

impl Inline<'_> {
    /// A cor, o tamanho e a posição do que sai agora com o `estilo`.
    fn fmt(&self, estilo: u8) -> Fmt {
        let mut tamanho = if self.e.tamanho > 0.0 { self.e.tamanho } else { self.ctx.classe.corpo };
        // A monoespaçada vai na mesma escala (o `\setmonofont[Scale=…]` do mesmo preâmbulo).
        if self.ctx.classe.escala != 1.0 && estilo & SERIFA == 0 {
            tamanho = if tamanho > 0.0 { tamanho } else { self.ctx.classe.tamanhos[4] } * self.ctx.classe.escala;
        }
        Fmt { cor: self.e.cor, tamanho, realce: self.e.realce, posicao: self.e.posicao, ..Fmt::default() }
    }

    fn texto(&mut self, t: &str, extra: u8) {
        if t.is_empty() {
            return;
        }
        let estilo = (self.e.estilo & !SERIFA) | self.e.familia.unwrap_or(self.ctx.classe.familia) | extra;
        let fmt = self.fmt(estilo);
        if let Some(u) = self.saida.last_mut() {
            if u.marca.is_none() && u.estilo == estilo && u.link == self.link && u.fmt == fmt {
                u.texto.push_str(t);
                return;
            }
        }
        self.saida.push(Trecho { texto: t.to_string(), estilo, link: self.link.clone(), fmt, ..Trecho::default() });
    }

    /// O mesmo que `texto`, mas sem espaço duplo.
    fn texto_colapsado(&mut self, t: &str) {
        if t == " " && self.saida.last().is_some_and(|u| u.texto.ends_with(' ')) {
            return;
        }
        self.texto(t, 0);
    }

    /// Corre `arg` num grupo: o que ele declarar não vaza.
    fn grupo(&mut self, arg: &str) {
        let antes = self.e;
        self.corre(arg);
        self.e = antes;
    }

    fn com_estilo(&mut self, bit: u8, arg: &str) {
        let antes = self.e;
        self.e.estilo |= bit;
        self.corre(arg);
        self.e = antes;
    }

    fn corre(&mut self, src: &str) {
        let mut i = 0;
        while i < src.len() {
            let resto = &src[i..];
            let c = resto.chars().next().unwrap_or(' ');
            i += match c {
                '\\' => self.barra(resto),
                '$' => self.matematica(resto),
                '~' => {
                    self.texto("\u{a0}", 0);
                    1
                }
                '{' => {
                    if let Some((dentro, k)) = delimitado(resto, '{', '}') {
                        self.grupo(dentro);
                        k
                    } else {
                        1
                    }
                }
                '}' => 1,
                '-' | '`' | '\'' => self.pontuacao(resto),
                c if c.is_whitespace() => {
                    let n = resto.len() - resto.trim_start().len();
                    self.texto_colapsado(" ");
                    n
                }
                _ => {
                    let n = resto.find(['\\', '$', '~', '{', '}', '-', '`', '\'']).filter(|&n| n > 0).unwrap_or(c.len_utf8());
                    let n = resto[..n].find(char::is_whitespace).filter(|&k| k > 0).unwrap_or(n);
                    self.texto(&resto[..n], 0);
                    n
                }
            };
        }
    }

    fn pontuacao(&mut self, resto: &str) -> usize {
        for (de, para) in [("---", "—"), ("--", "–"), ("``", "“"), ("''", "”")] {
            if resto.starts_with(de) {
                self.texto(para, 0);
                return de.len();
            }
        }
        let c = resto.chars().next().unwrap_or(' ');
        self.texto(&c.to_string(), 0);
        c.len_utf8()
    }

    fn matematica(&mut self, resto: &str) -> usize {
        let abre = if resto.starts_with("$$") { 2 } else { 1 };
        let corpo = &resto[abre..];
        match corpo.find(&resto[..abre]) {
            Some(f) => {
                // `$$…$$` no meio do parágrafo: fica no texto, mas no estilo display (o feather-math compõe).
                let formula = unidades::em_formula(corpo[..f].trim());
                let formula = formula.as_str();
                if abre == 2 {
                    self.texto(&format!("\\displaystyle {formula}"), MATEMATICA);
                } else {
                    self.texto(formula, MATEMATICA);
                }
                abre + f + abre
            }
            None => {
                self.texto("$", 0);
                1
            }
        }
    }

    /// `\(…\)`: fórmula em linha, como `$…$`; `resto` começa logo depois do `\(`.
    fn matematica_parenteses(&mut self, resto: &str) -> usize {
        match resto.find("\\)") {
            Some(f) => {
                self.texto(&unidades::em_formula(resto[..f].trim()), MATEMATICA);
                f + 2
            }
            None => 0,
        }
    }

    /// Declarações (valem até o fim do grupo): tamanho, série, forma e família. `true` se era uma.
    fn declaracao(&mut self, nome: &str) -> bool {
        if let Some(t) = self.ctx.classe.tamanho_de(nome) {
            self.e.tamanho = t;
            return true;
        }
        let e = &mut self.e.estilo;
        match nome {
            "bfseries" | "bf" => *e |= NEGRITO,
            "mdseries" => *e &= !NEGRITO,
            "itshape" | "it" | "slshape" | "sl" | "em" => *e |= ITALICO,
            "upshape" => *e &= !ITALICO,
            "ttfamily" | "tt" => *e |= CODIGO,
            "normalfont" | "rm" => *e &= !(NEGRITO | ITALICO | CODIGO),
            "sffamily" | "sf" => self.e.familia = Some(0),
            "rmfamily" => self.e.familia = Some(SERIFA),
            _ => return false,
        }
        true
    }

    /// Trata um comando em linha; devolve quantos bytes consumiu.
    fn barra(&mut self, resto: &str) -> usize {
        let (nome, n) = comando(resto);
        let depois = &resto[n..];
        if nome == "(" {
            return n + self.matematica_parenteses(depois);
        }
        if nome == "ensuremath" {
            if let Some((g, k)) = grupo(depois) {
                self.texto(g.trim(), MATEMATICA);
                return n + k;
            }
        }
        if matches!(nome, "\\" | "newline" | "linebreak") {
            let k = opcional(depois).map_or(0, |(_, len)| len);
            // A quebra de linha fica no parágrafo (Shift+Enter); a do começo ou do fim o `aparar` tira.
            if let Some(u) = self.saida.last_mut() {
                let a = u.texto.trim_end_matches(' ').len();
                u.texto.truncate(a);
            }
            self.texto(&featherdocs_core::modelo::QUEBRA_DE_LINHA.to_string(), 0);
            return n + k + depois[k..].len() - depois[k..].trim_start_matches([' ', '\n']).len();
        }
        if matches!(nome, "hspace" | "hspace*") {
            // O espaço vira espaços inquebráveis da mesma largura (o FeatherDocs não tem espaço medido).
            let k = opcional(depois).map_or(0, |(_, len)| len);
            if let Some((g, len)) = grupo(&depois[k..]) {
                if let Some(d) = dimensao(g, self.ctx.classe).filter(|d| *d > 0.0) {
                    let corpo = if self.e.tamanho > 0.0 { self.e.tamanho } else { self.ctx.classe.corpo };
                    let espacos = (d / (corpo * featherdocs_core::medida::largura(" ", 0, 1.0))).round().max(1.0) as usize;
                    self.texto(&"\u{a0}".repeat(espacos.min(200)), 0);
                }
                return n + k + len;
            }
            return n + k;
        }
        if matches!(nome, "vspace" | "setlength") {
            let mut k = 0;
            if let Some((_, len)) = opcional(&depois[k..]) {
                k += len;
            }
            if let Some((_, len)) = grupo(&depois[k..]) {
                k += len;
            }
            if nome == "setlength" {
                if let Some((_, len)) = grupo(&depois[k..]) {
                    k += len;
                }
            }
            return n + k;
        }
        if self.declaracao(nome) {
            return n;
        }
        if let Some((_, args)) = IGNORADOS.iter().find(|d| d.0 == nome) {
            let mut k = n;
            while let Some((_, len)) = opcional(&resto[k..]) {
                k += len;
            }
            for _ in 0..*args {
                if let Some((_, len)) = grupo(&resto[k..]) {
                    k += len;
                }
            }
            return k;
        }
        if let Some(r) = self.simbolo(nome, depois) {
            return n + r;
        }
        if let Some(r) = acento(nome, depois) {
            self.texto(&r.0, 0);
            return n + r.1;
        }
        if nome == "color" {
            if let Some((arg, g)) = grupo(depois) {
                if let Some(c) = resolver_cor(&self.ctx.cores, arg) {
                    self.e.cor = c;
                }
                return n + g;
            }
        }
        if nome == "textcolor" {
            if let Some(k) = self.textcolor(depois) {
                return n + k;
            }
        }
        if matches!(nome, "cite" | "citep" | "citet" | "parencite" | "textcite" | "autocite" | "footcite" | "citeonline" | "apud") {
            if let Some(k) = self.citacao(depois) {
                return n + k;
            }
        }
        if let Some(k) = self.extra(nome, depois) {
            return n + k;
        }
        // `\faCircleInfo`, `\faLightbulb[regular]`: o ícone, do tamanho da letra e na cor do texto.
        let cor = if self.e.cor == 0 { 0x000000FF } else { self.e.cor };
        if let Some(svg) = icones::svg(nome, &format!("#{:06x}", cor >> 8)) {
            let lado = 0.9 * if self.e.tamanho > 0.0 { self.e.tamanho } else { self.ctx.classe.tamanhos[4] };
            let origem = crate::base64::para_data_url("image/svg+xml", svg.as_bytes());
            self.saida.push(Trecho { estilo: self.e.estilo | self.ctx.classe.familia, ..Trecho::objeto(Marca::Imagem { origem, largura: lado, altura: lado }) });
            return n + if depois.starts_with('[') { opcional(depois).map_or(0, |o| o.1) } else { 0 };
        }
        // `\tikz[opções] …;` ou `\tikz{…}` no meio do texto: o desenho vira imagem na linha.
        if nome == "tikz" {
            let k = fim_do_tikz(depois);
            let codigo = depois[..k].trim();
            let codigo = grupo(codigo).filter(|(_, g)| *g == codigo.len()).map_or(codigo.to_string(), |(g, _)| g.to_string());
            let cores = self.ctx.cores.clone();
            let cor = move |c: &str| resolver_cor(&cores, c);
            match tikz::svg(&tikz::com_definicoes(&codigo, &self.ctx.tikz), &cor) {
                Some((svg, largura, altura, faltou)) => {
                    if faltou {
                        self.ctx.avisos.anotar(TipoDeAviso::SemEfeito, "tikz");
                    }
                    let escala = if self.e.escala > 0.0 { self.e.escala } else { 1.0 };
                    let origem = crate::base64::para_data_url("image/svg+xml", svg.as_bytes());
                    self.saida.push(Trecho::objeto(Marca::Imagem { origem, largura: largura * escala, altura: altura * escala }));
                }
                None => self.ctx.avisos.comando(nome),
            }
            return n + k;
        }
        let Some((arg, g)) = grupo(depois) else {
            self.ctx.avisos.comando(nome);
            // O `[…]` de um comando que não sabemos (`\faLightbulb[regular]`) é opção, não texto.
            return n + if depois.starts_with('[') { opcional(depois).map_or(0, |o| o.1) } else { 0 };
        };
        if self.com_argumento(nome, arg, &depois[g..]) {
            return n + g + extra_depois(nome, &depois[g..]);
        }
        // Comando desconhecido: o texto do argumento continua saindo, e o aviso diz o que faltou.
        self.ctx.avisos.comando(nome);
        self.grupo(arg);
        n + g
    }

    /// `\cite[antes][p.~4]{chave1,chave2}`: a citação vira objeto (a última opção é a página).
    fn citacao(&mut self, depois: &str) -> Option<usize> {
        let mut k = 0;
        let mut pagina = "";
        while let Some((o, len)) = opcional(&depois[k..]) {
            pagina = o;
            k += len;
        }
        let (chaves, g) = grupo(&depois[k..])?;
        let pagina = pagina.trim().trim_start_matches("pp.").trim_start_matches("p.").trim_start_matches('~').trim().replace('~', " ");
        let chaves = chaves.split(',').map(str::trim).filter(|c| !c.is_empty()).map(str::to_string).collect();
        let estilo = self.e.estilo | self.ctx.classe.familia;
        // Na letra do texto em volta (sem isto a citação sai no corpo padrão, fora da escala da letra).
        self.saida.push(Trecho { estilo, fmt: self.fmt(estilo), ..Trecho::objeto(Marca::Citacao { chaves, pagina }) });
        Some(k + g)
    }

    /// `\textcolor[modelo]{cor}{texto}`.
    fn textcolor(&mut self, depois: &str) -> Option<usize> {
        let (modelo, offset) = opcional(depois).map_or((None, 0), |(o, k)| (Some(o), k));
        let (cor_nome, g1) = grupo(&depois[offset..])?;
        let (texto, g2) = grupo(&depois[offset + g1..])?;
        let antes = self.e;
        let c = match modelo {
            Some(m) => parsear_cor(m, cor_nome),
            None => resolver_cor(&self.ctx.cores, cor_nome),
        };
        if let Some(cor) = c {
            self.e.cor = cor;
        }
        self.corre(texto);
        self.e = antes;
        Some(offset + g1 + g2)
    }

    /// Comandos sem argumento que viram um texto.
    fn simbolo(&mut self, nome: &str, depois: &str) -> Option<usize> {
        let t = match nome {
            "newline" | "linebreak" => " ",
            "ldots" | "dots" | "textellipsis" => "…",
            "LaTeX" => "LaTeX",
            "TeX" => "TeX",
            "S" => "§",
            "copyright" => "©",
            "textbackslash" => "\\",
            "textbullet" => "•",
            "textbar" => "|",
            "textendash" => "–",
            "textemdash" => "—",
            "quad" | "qquad" | " " | "," | ";" | "!" | ":" | "enspace" | "thinspace" => " ",
            "hfill" | "hfil" => "\t",
            "&" | "%" | "$" | "#" | "_" | "{" | "}" => nome,
            "noindent" | "indent" | "centering" | "raggedright" | "raggedleft" | "par" | "label" | "protect" | "relax" | "strut" | "null" | "nobreak" | "/" => "",
            _ => return None,
        };
        if t == "\t" {
            self.texto(t, 0);
        } else {
            self.texto_colapsado(t);
        }
        Some(if nome == "label" { grupo(depois).map_or(0, |g| g.1) } else { 0 })
    }

    /// Comandos com um argumento `{…}`. `true` se tratou (e já consumiu o argumento).
    fn com_argumento(&mut self, nome: &str, arg: &str, depois: &str) -> bool {
        match nome {
            "textbf" | "mathbf" => self.com_estilo(NEGRITO, arg),
            "emph" | "textit" | "textsl" | "mathit" => self.com_estilo(ITALICO, arg),
            "underline" | "uline" => self.com_estilo(SUBLINHADO, arg),
            "sout" | "st" => self.com_estilo(TACHADO, arg),
            // Versalete: as minúsculas viram maiúsculas menores (80%).
            "textsc" => {
                let n = self.saida.len();
                self.corre(arg);
                let novos: Vec<Trecho> = self.saida.drain(n..).collect();
                for t in novos {
                    if t.marca.is_some() || t.estilo & MATEMATICA != 0 {
                        self.saida.push(t);
                        continue;
                    }
                    let base = if t.fmt.tamanho > 0.0 { t.fmt.tamanho } else { featherdocs_core::layout::CORPO };
                    let mut atual = String::new();
                    let mut minuscula = false;
                    let empurrar = |texto: &mut String, minuscula: bool, saida: &mut Vec<Trecho>| {
                        if !texto.is_empty() {
                            let mut novo = Trecho { texto: std::mem::take(texto), ..t.clone() };
                            if minuscula {
                                novo.texto = novo.texto.to_uppercase();
                                novo.fmt.tamanho = (base * 0.8 * 10.0).round() / 10.0;
                            }
                            saida.push(novo);
                        }
                    };
                    for c in t.texto.chars() {
                        if c.is_lowercase() != minuscula && !atual.is_empty() {
                            empurrar(&mut atual, minuscula, &mut self.saida);
                        }
                        minuscula = c.is_lowercase();
                        atual.push(c);
                    }
                    empurrar(&mut atual, minuscula, &mut self.saida);
                }
            }
            // `\rule{largura}{altura}` no meio do texto: o traço para preencher ("Nome: ____").
            // `\\rule{largura}{altura}`: o traço desenhado, na linha de base.
            "rule" => {
                let largura = self.medida_na_linha(arg);
                let altura = grupo(depois).and_then(|(h, _)| dimensao(h, self.ctx.classe)).unwrap_or(0.4).max(0.1);
                if largura > 0.0 {
                    let cor = format!("#{:06x}", if self.e.cor == 0 { 0 } else { self.e.cor >> 8 });
                    self.desenho(&format!("<rect width='{largura}' height='{altura}' fill='{cor}'/>"), largura, altura);
                }
            }
            // As linhas e a caixa da resposta da classe `exam` (ver `exam.rs`).
            // `{altura}{passo}`: as linhas inteiras que cabem na altura (o `\\fillwithlines`), o resto em branco.
            "featherexamlinhas" => {
                let altura = arg.trim().parse::<f32>().unwrap_or(72.0).clamp(1.0, 2000.0);
                let passo = grupo(depois).and_then(|(p, _)| p.trim().parse::<f32>().ok()).unwrap_or(18.0).max(1.0);
                let n = ((altura / passo) - 1e-3).floor().max(1.0) as usize;
                let largura = self.medida_na_linha("\\linewidth");
                let fios: String = (1..=n).map(|k| format!("<rect y='{}' width='{largura}' height='0.4'/>", k as f32 * passo - 0.4)).collect();
                self.desenho(&fios, largura, altura.max(n as f32 * passo));
            }
            "featherexamcaixa" => {
                let altura = arg.trim().parse::<f32>().unwrap_or(72.0);
                let largura = self.medida_na_linha("\\linewidth");
                self.desenho(&format!("<rect x='0.2' y='0.2' width='{}' height='{}' fill='none' stroke='#000' stroke-width='0.4'/>", largura - 0.4, altura - 0.4), largura, altura);
            }
            // `\fbox` no meio do texto: o conteúdo (a moldura de uma linha não tem como sair).
            "fbox" | "framebox" | "mbox" | "makebox" => self.corre(arg),
            // `\scalebox{fator}{…}`: o texto no tamanho dele; o fator vale para o desenho do `\tikz`.
            "scalebox" => {
                let antes = self.e.escala;
                self.e.escala = if antes > 0.0 { antes } else { 1.0 } * arg.trim().parse::<f32>().unwrap_or(1.0);
                self.corre(grupo(depois).map_or("", |g| g.0));
                self.e.escala = antes;
            }
            // `censor`: a tarja preta por cima do texto.
            "censor" => {
                let antes = self.e;
                (self.e.realce, self.e.cor) = (0x000000FF, 0x000000FF);
                self.corre(arg);
                self.e = antes;
            }
            "textsf" | "textrm" => {
                let antes = self.e;
                self.e.familia = Some(if nome == "textrm" { SERIFA } else { 0 });
                self.corre(arg);
                self.e = antes;
            }
            // O `\\texttt` com comando dentro (`\\texttt{$\\backslash$}`) se lê; sem, fica como está (o `--` não vira travessão).
            "texttt" if arg.contains(['\\', '$']) => self.com_estilo(CODIGO, arg),
            "texttt" | "code" | "verb" | "lstinline" => self.texto(arg, CODIGO),
            "MakeUppercase" | "uppercase" => self.trocar_caixa(arg, true),
            "MakeLowercase" | "lowercase" => self.trocar_caixa(arg, false),
            "url" => {
                let (antes, familia) = (self.link.replace(arg.to_string()), self.e.familia);
                match self.ctx.estilo_url.as_str() {
                    "same" => self.texto(arg, 0),
                    "sf" | "rm" => {
                        self.e.familia = Some(if self.ctx.estilo_url == "rm" { SERIFA } else { 0 });
                        self.texto(arg, 0);
                    }
                    _ => self.texto(arg, CODIGO),
                }
                (self.link, self.e.familia) = (antes, familia);
            }
            "href" => {
                let texto = grupo(depois).map_or("", |g| g.0);
                self.com_link(arg, texto);
            }
            "ref" if self.ctx.rotulos.contains_key(arg.trim()) => self.com_link(&format!("#rotulo:{}", arg.trim()), &self.ctx.rotulos[arg.trim()].0.clone()),
            "eqref" if self.ctx.rotulos.contains_key(arg.trim()) => self.com_link(&format!("#rotulo:{}", arg.trim()), &format!("({})", self.ctx.rotulos[arg.trim()].0)),
            // `cleveref` (e o `\autoref` do hyperref): o nome do que o rótulo numera vem junto ("seção 3").
            "cref" | "Cref" | "autoref" | "Autoref" if arg.split(',').all(|k| self.ctx.rotulos.contains_key(k.trim())) => {
                let partes: Vec<String> = arg.split(',').map(|k| {
                    let (n, tipo) = &self.ctx.rotulos[k.trim()];
                    // O `\autoref` do `hyperref` (com o `babel`) dá "Figura 1", "Seção 2", "Subseção 1.1".
                    let tipo = &if nome.eq_ignore_ascii_case("autoref") {
                        let fundo = n.matches('.').count().saturating_sub(usize::from(self.ctx.capitulo));
                        let tipo = if tipo == "seção" { format!("{}seção", "sub".repeat(fundo.min(2))) } else { tipo.clone() };
                        maiuscula_inicial(&tipo)
                    } else {
                        tipo.clone()
                    };
                    if tipo == "equação" { format!("{tipo} ({n})") } else if tipo.is_empty() { n.clone() } else { format!("{tipo} {n}") }
                }).collect();
                let mut texto = partes.join(" e ");
                if nome.starts_with(char::is_uppercase) {
                    texto = maiuscula_inicial(&texto);
                }
                // O link vai para o primeiro rótulo (o `cleveref` liga cada número ao seu).
                self.com_link(&format!("#rotulo:{}", arg.split(',').next().unwrap_or("").trim()), &texto);
            }
            // O rótulo que não existe sai como no LaTeX ("??") e vai para o aviso.
            "ref" | "eqref" | "autoref" | "cref" | "Cref" | "pageref" => {
                self.ctx.avisos.anotar(TipoDeAviso::Comando, &format!("\\{nome}{{{}}}", arg.trim()));
                self.texto("??", NEGRITO);
            }
            "citeauthor" | "citeyear" if arg.split(',').all(|k| self.ctx.fontes.contains_key(k.trim())) => {
                let partes: Vec<String> = arg.split(',').map(|k| {
                    let (autores, ano) = &self.ctx.fontes[k.trim()];
                    if nome == "citeauthor" { autores.clone() } else { ano.clone() }
                }).collect();
                // Como o hyperref: o autor citado leva à lista de referências.
                self.com_link("#rotulo:@referencias", &partes.join("; "));
            }
            "cite" | "citep" | "citet" | "citeauthor" | "citeyear" => self.texto(&format!("[{arg}]"), 0),
            "footnote" | "endnote" | "thanks" => {
                let mut nota = Inline { saida: Vec::new(), e: Estado::default(), link: None, ctx: self.ctx, aspas: 0 };
                nota.corre(arg);
                let texto = aparar(nota.saida);
                self.saida.push(Trecho { estilo: self.e.estilo | self.ctx.classe.familia, ..Trecho::objeto(Marca::Nota { fim: nome == "endnote", texto }) });
            }
            "index" => {
                let (termo, sub) = arg.split_once('!').unwrap_or((arg, ""));
                self.saida.push(Trecho::objeto(Marca::Indice { termo: termo.trim().to_string(), sub: sub.trim().to_string() }));
            }
            "label" | "phantom" | "hphantom" | "vphantom" => {}
            _ => return false,
        }
        true
    }

    /// `\MakeUppercase{…}`: o argumento é lido normalmente (comandos e tudo) e só o texto que ele
    /// produziu muda de caixa.
    fn trocar_caixa(&mut self, arg: &str, maiusculas: bool) {
        let n = self.saida.len();
        let ja = self.saida.last().map_or(0, |t| t.texto.len());
        self.grupo(arg);
        let trocar = |t: &str| if maiusculas { t.to_uppercase() } else { t.to_lowercase() };
        for (k, t) in self.saida.iter_mut().enumerate().skip(n.saturating_sub(1)) {
            if t.estilo & MATEMATICA != 0 {
                continue;
            }
            if k + 1 == n {
                let (fica, muda) = t.texto.split_at(ja.min(t.texto.len()));
                t.texto = format!("{fica}{}", trocar(muda));
            } else {
                t.texto = trocar(&t.texto);
            }
        }
    }

    /// Uma medida do texto (`3cm`, `.5\\linewidth`); o `\\linewidth` dentro de lista desconta o recuo dela.
    fn medida_na_linha(&self, v: &str) -> f32 {
        match v.trim().strip_suffix("\\linewidth") {
            Some(f) => {
                let recuo = if self.ctx.nivel_lista > 0 { self.ctx.classe.recuo_da_lista(self.ctx.nivel_lista - 1) } else { 0.0 };
                f.trim().parse::<f32>().unwrap_or(1.0) * (self.ctx.largura_util - recuo)
            }
            None => comprimento(v, self.ctx).unwrap_or(0.0),
        }
        .max(0.0)
    }

    /// Um desenho SVG (`corpo` em pontos) de `largura` × `altura` na linha, como imagem.
    fn desenho(&mut self, corpo: &str, largura: f32, altura: f32) {
        let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' width='{largura}' height='{altura}' viewBox='0 0 {largura} {altura}'>{corpo}</svg>");
        let origem = crate::base64::para_data_url("image/svg+xml", svg.as_bytes());
        self.saida.push(Trecho::objeto(Marca::Imagem { origem, largura, altura }));
    }

    fn com_link(&mut self, url: &str, texto: &str) {
        let antes = self.link.replace(url.to_string());
        self.grupo(texto);
        self.link = antes;
    }
}

/// `\href{url}{texto}` consome o segundo grupo também.
fn extra_depois(nome: &str, depois: &str) -> usize {
    if nome == "href" || nome == "rule" || nome == "scalebox" || nome == "featherexamlinhas" {
        grupo(depois).map_or(0, |g| g.1)
    } else {
        0
    }
}

/// Acentos: `\'a`, `\'{a}`, `\~a`, `\c{c}`, `\c c`… Devolve o caractere e quantos bytes leu depois do comando.
fn acento(nome: &str, depois: &str) -> Option<(String, usize)> {
    let marca = match nome {
        "'" => '\u{301}',
        "`" => '\u{300}',
        "^" => '\u{302}',
        "\"" => '\u{308}',
        "~" => '\u{303}',
        "c" => '\u{327}',
        _ => return None,
    };
    // `\'{\i}` e `\'\i`: o "i sem pingo" leva o acento (í).
    let sem_pingo = |t: &str| ["\\i", "\\j"].iter().find(|p| t.starts_with(**p) && !t[2..].starts_with(|c: char| c.is_ascii_alphabetic())).map(|p| if *p == "\\i" { 'i' } else { 'j' });
    let (base, lido) = if let Some((g, n)) = grupo(depois) { (sem_pingo(g.trim()).or_else(|| g.chars().next())?, n) } else {
        let t = depois.trim_start();
        match sem_pingo(t) {
            Some(c) => (c, depois.len() - t.len() + 2),
            None => (t.chars().next()?, depois.len() - t.len() + t.chars().next()?.len_utf8()),
        }
    };
    Some((compor(base, marca), lido))
}

/// Letra + acento já composta (á, ç, ñ…); sem forma pronta, devolve só a letra.
fn compor(base: char, marca: char) -> String {
    let tabela = [
        ("aáàâäã", 'a'),
        ("eéèêë~", 'e'),
        ("iíìîï~", 'i'),
        ("oóòôöõ", 'o'),
        ("uúùûü~", 'u'),
        ("AÁÀÂÄÃ", 'A'),
        ("EÉÈÊË~", 'E'),
        ("IÍÌÎÏ~", 'I'),
        ("OÓÒÔÖÕ", 'O'),
        ("UÚÙÛÜ~", 'U'),
    ];
    let posicao = match marca {
        '\u{301}' => 1,
        '\u{300}' => 2,
        '\u{302}' => 3,
        '\u{308}' => 4,
        '\u{303}' => 5,
        _ => 0,
    };
    if marca == '\u{327}' {
        return match base {
            'c' => "ç".into(),
            'C' => "Ç".into(),
            _ => base.to_string(),
        };
    }
    if marca == '\u{303}' && (base == 'n' || base == 'N') {
        return if base == 'n' { "ñ".into() } else { "Ñ".into() };
    }
    for (letras, b) in tabela {
        if b == base && posicao > 0 {
            return letras.chars().nth(posicao).filter(|&c| c != '~').map_or_else(|| base.to_string(), |c| c.to_string());
        }
    }
    base.to_string()
}

// ---------------------------------------------------------------------------
// Blocos
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Ctx {
    capitulo: bool,
    /// O separador depois de "Tabela 1" nas legendas: `:` nas classes padrão (e no `labelsep` do
    /// `\captionsetup`); vazio = o " –" do FeatherDocs (a ABNT).
    separador_legenda: &'static str,
    nivel_lista: u8,
    titulo: String,
    autor: String,
    fundo: u32,
    alinhamento: Alinhamento,
    cores: HashMap<String, u32>,
    classe: Classe,
    /// As declarações em vigor neste grupo.
    estado: Estado,
    /// `\parskip`, quando o documento o define.
    parskip: Option<f32>,
    /// O vão entre itens (`\itemsep` + `\parsep`) de `itemize` e de `enumerate`.
    itemsep: [f32; 2],
    cor_marcador: [u32; 2],
    /// `\vspace` em espera: vira o espaço antes do próximo bloco.
    vspace: f32,
    /// `\vfill` em espera: o próximo bloco ganha o espaço elástico antes (`Atrib::encher`).
    encher: bool,
    /// O espaço do fim da lista ou do `center` que acabou de fechar (o `\addvspace` do LaTeX).
    fim_de_ambiente: f32,
    /// O que o leitor não entendeu (compartilhado com os analisadores filhos).
    avisos: avisos::Coletor,
    /// Chave da fonte → (autores como o texto os cita, ano).
    fontes: std::rc::Rc<HashMap<String, (String, String)>>,
    /// Rótulo do `\label` → o número que o `\ref` mostra e o nome do que ele numera ("seção", "figura").
    rotulos: std::rc::Rc<HashMap<String, (String, String)>>,
    /// A largura do texto em pontos (o `\linewidth` das imagens).
    largura_util: f32,
    /// `\captionsetup{font=bf}`: a legenda inteira ("Figura 1 – …") em negrito.
    legenda_negrito: bool,
    /// `\renewcommand{\arraystretch}{1.5}`: quanto as linhas da tabela esticam (0 = o normal).
    estica_tabela: f32,
    /// `\rowcolors{2}{a}{b}`: a cor que pinta uma linha sim, outra não, e se são as pares.
    zebra: Option<(u32, bool)>,
    /// `\title`, `\author` e `\date` como estão no código (o `\maketitle` os compõe).
    titulo_fonte: String,
    autor_fonte: String,
    data_fonte: Option<String>,
    /// `\setstretch`, `\onehalfspacing`, `\doublespacing`: quanto a entrelinha do texto estica.
    espaco: f32,
    /// Dentro de `figure`: o `\caption` é de figura; dentro de `subfigure`, a letra dela.
    em_figura: bool,
    subfigura: Option<char>,
    proxima_subfigura: std::rc::Rc<std::cell::Cell<u8>>,
    /// `\newtheorem` e `\newtcolorbox` do preâmbulo, e os números dos teoremas na ordem do texto.
    teoremas: std::rc::Rc<HashMap<String, pacotes::Teorema>>,
    numeros_teoremas: std::rc::Rc<std::cell::RefCell<std::collections::VecDeque<String>>>,
    caixas: std::rc::Rc<HashMap<String, pacotes::Caixa>>,
    /// As listas do `\newlist` (o `artigo` de uma norma): numerada ou não e as opções de cada nível.
    listas_novas: std::rc::Rc<HashMap<String, (bool, Vec<String>)>>,
    /// As listas do `\newlist` abertas em volta (o nível de cada uma conta à parte, como no `enumitem`).
    listas_abertas: Vec<String>,
    /// Quantos `itemize` e quantos `enumerate` estão abertos (o rótulo padrão do LaTeX vai por eles).
    profundidade: [u8; 2],
    /// A parte do `vspace` em espera que veio no meio do parágrafo (`texto\\bigskip`): o TeX a põe
    /// depois da linha, e o `\\topsep` da lista seguinte soma a ela em vez de ficar com o maior.
    vspace_em_linha: f32,
    /// O preâmbulo troca os rótulos padrão (`\\labelitemi`, `\\setlist[itemize]{label=…}`): o FeatherDocs não põe os do LaTeX.
    rotulos_do_preambulo: bool,
    /// O número de cada `lstlisting` com legenda, na ordem do texto.
    listagens: std::rc::Rc<std::cell::Cell<u32>>,
    listings_utf8: bool,
    /// A linguagem de cada estilo do `\lstdefinestyle{bash}{language=bash,…}`.
    estilos_lst: std::rc::Rc<(String, HashMap<String, String>)>,
    /// Os estilos, `pics` e variáveis que o preâmbulo define para os desenhos TikZ.
    tikz: std::rc::Rc<tikz::Definicoes>,
    /// Os tipos de coluna do `\newcolumntype`: a letra → (argumentos, o que ela vale).
    tipos_de_coluna: std::rc::Rc<HashMap<char, (usize, String)>>,
    /// O `\urlstyle{…}` do preâmbulo; vazio = o `tt` do pacote `url`.
    estilo_url: String,
    /// O `tabsize` do `listings` (8 se o preâmbulo não diz).
    tabulacao: usize,
}

/// O quanto a entrelinha estica pelo `setspace` (`\onehalfspacing`, `\doublespacing`, `\setstretch{x}`)
/// ou pelo `\linespread{x}`; 0 se nada a muda. Vale o último.
fn espaco_do(s: &str) -> f32 {
    let mut espaco = 0.0;
    let mut i = 0;
    while let Some(p) = s[i..].find('\\') {
        i += p;
        let (nome, n) = comando(&s[i..]);
        i += n;
        espaco = match nome {
            "onehalfspacing" | "OnehalfSpacing" => 1.25,
            "doublespacing" | "DoubleSpacing" => 1.667,
            "singlespacing" | "SingleSpacing" => 1.0,
            "setstretch" | "linespread" => grupo(&s[i..]).and_then(|(g, _)| g.trim().parse().ok()).unwrap_or(espaco),
            _ => espaco,
        };
    }
    espaco
}

/// O `\arraystretch` de um `\renewcommand{\arraystretch}{1.5}` (ou sem chaves no nome); 0 se não há.
fn estica_do(s: &str) -> f32 {
    let Some(p) = s.find("\\arraystretch") else { return 0.0 };
    let depois = s[p + 13..].trim_start_matches('}');
    grupo(depois).and_then(|(g, _)| g.trim().parse().ok()).unwrap_or(0.0)
}

/// A largura do `\includegraphics[width=…]` em pontos (0 = a natural); `0.8\linewidth` é fração da largura do texto.
fn largura_da_imagem(opcoes: &str, ctx: &Ctx) -> f32 {
    let o = opcoes_chave_valor(opcoes);
    let Some((_, v)) = o.iter().find(|(k, _)| k == "width") else {
        // `scale=0.7`: a fração do tamanho natural, que só o arquivo diz (vai negativa até lá).
        return o.iter().find(|(k, _)| k == "scale").and_then(|(_, v)| v.trim().parse::<f32>().ok()).filter(|e| *e > 0.0).map_or(0.0, |e| -e);
    };
    let v = v.clone();
    // `0.8\textwidth` é como o FeatherDocs escreve a imagem sem tamanho próprio (a do arquivo).
    if v.replace(' ', "") == "0.8\\textwidth" {
        return 0.0;
    }
    comprimento(&v, ctx).unwrap_or(0.0)
}

/// Uma medida em pontos: `0.48\textwidth` (a largura do texto em volta), `3cm`, `12pt`.
fn comprimento(v: &str, ctx: &Ctx) -> Option<f32> {
    for linha in ["\\linewidth", "\\textwidth", "\\columnwidth", "\\hsize"] {
        if let Some(f) = v.trim().strip_suffix(linha) {
            return Some(f.trim().parse::<f32>().unwrap_or(1.0) * ctx.largura_util);
        }
    }
    dimensao(v, ctx.classe)
}

/// Os títulos que o preâmbulo põe em maiúsculas: a opção `section=TITLE` da abnTeX2 ou o
/// `\titleformat{\section}{…\MakeUppercase}` do `titlesec`.
fn titulos_em_maiusculas(doc: &mut Documento, preambulo: &str, capitulo: bool) {
    let opcoes = preambulo.find("\\documentclass").and_then(|i| opcional(&preambulo[i + 14..])).map_or(String::new(), |(o, _)| o.replace([' ', '\n', '\t'], ""));
    let mut niveis = Vec::new();
    for (k, divisao) in ["chapter", "section", "subsection", "subsubsection"].iter().enumerate() {
        let formato = format!("\\titleformat{{\\{divisao}}}");
        let pelo_titlesec = preambulo.find(&formato).and_then(|i| grupo(&preambulo[i + formato.len()..])).is_some_and(|(g, _)| g.contains("\\MakeUppercase"));
        if opcoes.contains(&format!("{divisao}=TITLE")) || pelo_titlesec {
            // O nível do título no documento: com `\chapter`, a seção é o nível 2.
            niveis.push(u8::try_from(k).unwrap_or(0) + u8::from(capitulo || k == 0));
        }
    }
    for b in &mut doc.blocos {
        if let Conteudo::Titulo { nivel, texto } = &mut b.conteudo {
            if niveis.contains(nivel) {
                for t in texto.iter_mut().filter(|t| t.marca.is_none() && t.estilo & MATEMATICA == 0) {
                    t.texto = t.texto.to_uppercase();
                }
            }
        }
    }
}

/// O `\titleformat{\chapter}[display]{…\centering}{\MakeUppercase{\chaptertitlename}\ \thechapter}…` do
/// `titlesec`: o rótulo ("CAPÍTULO I") numa linha e o nome na de baixo. O rótulo entra no texto do título,
/// que sai sem o número do motor; só quando todas as divisões numeradas são assim, senão a contagem do
/// motor ficaria sem os capítulos.
fn titulos_em_bloco(doc: &mut Documento, preambulo: &str, capitulo: bool) {
    let mut rotulos: [Option<(String, bool)>; 7] = Default::default();
    for (divisao, sem, com) in DIVISOES {
        let nivel = usize::from(if capitulo { com } else if divisao == "chapter" { continue } else { sem });
        let formato = format!("\\titleformat{{\\{divisao}}}");
        let Some(i) = preambulo.find(&formato) else { continue };
        let resto = &preambulo[i + formato.len()..];
        let Some((_, n)) = opcional(resto).filter(|(f, _)| f.trim() == "display") else { continue };
        let resto = &resto[n..];
        let Some((estilo, n)) = grupo(resto) else { continue };
        let Some((rotulo, _)) = grupo(&resto[n..]) else { continue };
        rotulos[nivel] = Some((rotulo_da_divisao(rotulo, preambulo), estilo.contains("\\centering")));
    }
    let todos = doc.blocos.iter().all(|b| match &b.conteudo {
        Conteudo::Titulo { nivel, .. } => b.atrib.sem_numero || rotulos[usize::from((*nivel).min(6))].is_some(),
        _ => true,
    });
    if !todos || rotulos.iter().all(Option::is_none) {
        return;
    }
    let compacto = preambulo.replace([' ', '\n'], "");
    let mut contadores = [0u32; 7];
    for i in 0..doc.blocos.len() {
        let depois_do_anterior = i.checked_sub(1).and_then(|j| doc.blocos[j].atrib.depois).unwrap_or(0.0);
        let b = &mut doc.blocos[i];
        let Conteudo::Titulo { nivel, texto } = &mut b.conteudo else { continue };
        let n = usize::from((*nivel).min(6));
        let Some((modelo, centro)) = &rotulos[n] else { continue };
        if *centro {
            b.atrib.alinhamento = Alinhamento::Centro;
        }
        // O `\chapter` do `titlesec` abre com `\vspace*`: o espaço de antes soma com o do bloco de cima.
        if capitulo && n == 1 {
            let antes = featherdocs_core::modelo::Estilo::do_latex(1, &b.latex).and_then(|e| e.antes).unwrap_or(0.0);
            b.atrib.antes = Some(antes + depois_do_anterior.max(b.atrib.antes.unwrap_or(0.0)));
        }
        if b.atrib.sem_numero {
            continue;
        }
        contadores[n] += 1;
        for c in &mut contadores[n + 1..] {
            *c = 0;
        }
        let mut rotulo = modelo.clone();
        for (divisao, sem, com) in DIVISOES {
            let k = usize::from(if capitulo { com } else if divisao == "chapter" { continue } else { sem });
            let estilo = ["Roman", "roman", "Alph", "alph"].into_iter().find(|e| compacto.contains(&format!("\\the{divisao}}}{{\\{e}{{{divisao}}}}}")) || compacto.contains(&format!("\\the{divisao}{{\\{e}{{{divisao}}}}}")));
            let c = contadores[k];
            let numero = match estilo {
                Some("Roman") => featherdocs_core::paginacao::romano(c),
                Some("roman") => featherdocs_core::paginacao::romano(c).to_lowercase(),
                Some("Alph") => featherdocs_core::paginacao::letra(c),
                Some("alph") => featherdocs_core::paginacao::letra(c).to_lowercase(),
                _ => c.to_string(),
            };
            rotulo = rotulo.replace(&format!("\\the{divisao}"), &numero);
        }
        let primeiro = texto.first().cloned().unwrap_or_default();
        texto.insert(0, Trecho { texto: format!("{}{}", rotulo.trim(), featherdocs_core::modelo::QUEBRA_DE_LINHA), link: None, marca: None, ..primeiro });
        b.atrib.sem_numero = true;
    }
}

/// O rótulo do `\titleformat` em texto, com o `\the…` da divisão ainda por trocar pelo número.
fn rotulo_da_divisao(rotulo: &str, preambulo: &str) -> String {
    let nome = if preambulo.contains("brazil") || preambulo.contains("portugu") { "Capítulo" } else { "Chapter" };
    let mut r = rotulo.replace("\\chaptertitlename", nome).replace("\\chaptername", nome).replace("\\ ", " ").replace('~', " ");
    while let Some(i) = r.find("\\MakeUppercase") {
        let fim = grupo(&r[i + 14..]).map_or(i + 14, |(_, k)| i + 14 + k);
        let dentro = grupo(&r[i + 14..]).map_or(String::new(), |(g, _)| g.to_uppercase());
        r.replace_range(i..fim, &dentro);
    }
    r.replace(['{', '}'], "")
}

/// `\newcolumntype{R}[1]{>{\RaggedLeft\arraybackslash}p{#1}}`: a letra, quantos argumentos e o que ela vale.
fn tipos_de_coluna(preambulo: &str) -> HashMap<char, (usize, String)> {
    let mut saida = HashMap::new();
    for (i, _) in preambulo.match_indices("\\newcolumntype") {
        let resto = &preambulo[i + 14..];
        let Some((letra, mut k)) = grupo(resto) else { continue };
        let args = opcional(&resto[k..]).map_or(0, |(n, len)| {
            k += len;
            n.trim().parse().unwrap_or(0)
        });
        if let (Some(c), Some((def, _))) = (letra.trim().chars().next(), grupo(&resto[k..])) {
            saida.insert(c, (args, def.to_string()));
        }
    }
    saida
}

/// O `{l p{2cm} R{6cm}}` do começo do corpo com os tipos do `\newcolumntype` trocados pelo que valem.
fn expandir_tipos_de_coluna(corpo: &str, tipos: &HashMap<char, (usize, String)>) -> String {
    let Some((spec, k)) = grupo(corpo).filter(|_| !tipos.is_empty() && corpo.starts_with('{')) else { return corpo.to_string() };
    let mut saida = String::new();
    let mut j = 0;
    while let Some(c) = spec[j..].chars().next() {
        j += c.len_utf8();
        match tipos.get(&c) {
            Some((args, def)) => {
                let mut def = def.clone();
                for n in 1..=*args {
                    let Some((a, len)) = grupo(spec[j..].trim_start()) else { break };
                    j += spec[j..].len() - spec[j..].trim_start().len() + len;
                    def = def.replace(&format!("#{n}"), a);
                }
                saida.push_str(&def);
            }
            None => {
                // O que vem num `{…}` (`p{2cm}`, `@{}`) não é tipo de coluna.
                saida.push(c);
                if let Some((g, len)) = grupo(&spec[j..]).filter(|_| spec[j..].starts_with('{')) {
                    saida.push_str(&format!("{{{g}}}"));
                    j += len;
                }
            }
        }
    }
    format!("{{{saida}}}{}", &corpo[k..])
}

/// `\lstdefinestyle{nome}{language=bash,…}`: a linguagem de cada estilo (o que herda de outro estilo
/// sem dizer a linguagem fica de fora).
/// Uma declaração de letra (`\\footnotesize\\bfseries\\color{x}`): o corpo, `NEGRITO`/`ITALICO`, a
/// família (`0` sem serifa, `SERIFA`, `CODIGO`) e a cor, cada um só se a declaração o diz.
fn letra(f: &str, classe: Classe, cores: &HashMap<String, u32>) -> (Option<f32>, u8, Option<u8>, Option<u32>) {
    let (mut tamanho, mut bits, mut familia, mut cor) = (None, 0, None, None);
    let mut k = 0;
    while let Some(p) = f[k..].find('\\') {
        k += p;
        let (nome, n) = comando(&f[k..]);
        k += n.max(1);
        match nome {
            "bfseries" => bits |= NEGRITO,
            "itshape" | "slshape" => bits |= ITALICO,
            "sffamily" => familia = Some(0),
            "rmfamily" => familia = Some(SERIFA),
            "ttfamily" => familia = Some(CODIGO),
            "color" => cor = grupo(&f[k..]).and_then(|(c, _)| resolver_cor(cores, c)).or(cor),
            _ => tamanho = classe.tamanho_de(nome).map(|t| if t == 0.0 { classe.tamanhos[4] } else { t }).or(tamanho),
        }
    }
    (tamanho, bits, familia, cor)
}

/// Os `\\lstset{…}` do preâmbulo, juntos, e as opções de cada `\\lstdefinestyle{nome}{…}`.
fn estilos_lst(preambulo: &str) -> (String, HashMap<String, String>) {
    let mut estilos = HashMap::new();
    for (i, _) in preambulo.match_indices("\\lstdefinestyle") {
        let resto = &preambulo[i + 15..];
        let Some((nome, k)) = grupo(resto) else { continue };
        let Some((o, _)) = grupo(&resto[k..]) else { continue };
        estilos.insert(nome.trim().to_string(), o.to_string());
    }
    let global = preambulo.match_indices("\\lstset").filter_map(|(i, _)| grupo(&preambulo[i + 7..]).map(|(o, _)| o)).collect::<Vec<_>>().join(",");
    (global, estilos)
}

/// As opções em vigor numa listagem, na ordem (a última vale): as do `\\lstset` e depois as dela,
/// com cada `style=` trocado pelas opções do estilo.
fn opcoes_lst(estilos: &(String, HashMap<String, String>), locais: &str) -> Vec<(String, String)> {
    fn abrir(o: &str, estilos: &HashMap<String, String>, saida: &mut Vec<(String, String)>, nivel: u8) {
        for (k, v) in opcoes_chave_valor(o) {
            match estilos.get(v.trim_matches(['{', '}']).trim()) {
                Some(e) if k == "style" && nivel < 8 => abrir(e, estilos, saida, nivel + 1),
                _ if k.is_empty() => {}
                _ => saida.push((k, v)),
            }
        }
    }
    let mut saida = Vec::new();
    abrir(&estilos.0, &estilos.1, &mut saida, 0);
    abrir(locais, &estilos.1, &mut saida, 0);
    saida
}

/// A letra de cada nível de título (índice = nível) como a classe `article`/`report` a faz: `\Large`
/// na seção, `\large` na subseção, negrito, na família do texto, com os espaços de antes e depois
/// (`3.5ex`/`2.3ex` na seção, `3.25ex`/`1.5ex` abaixo dela); o `\titleformat{…}{…}` troca tamanho,
/// negrito, itálico, família e cor. Formato de `Estilo::do_latex`.
fn formatos_dos_titulos(preambulo: &str, classe: Classe, cores: &HashMap<String, u32>, capitulo: bool) -> [String; 7] {
    let mut saida: [String; 7] = Default::default();
    let ex = classe.tamanhos[4] * 0.43;
    for (divisao, sem, com) in DIVISOES {
        let nivel = usize::from(if capitulo { com } else if divisao == "chapter" { continue } else { sem });
        let (mut tamanho, mut bits, mut cor) = (match divisao {
            "chapter" => classe.tamanhos[9],
            "section" => classe.tamanhos[6],
            "subsection" => classe.tamanhos[5],
            _ => classe.tamanhos[4],
        }, NEGRITO | classe.familia, 0u32);
        let formato = format!("\\titleformat{{\\{divisao}}}");
        if let Some(i) = preambulo.find(&formato) {
            let mut resto = &preambulo[i + formato.len()..];
            resto = opcional(resto).map_or(resto, |(_, n)| &resto[n..]);
            if let Some((f, _)) = grupo(resto) {
                let (t, b, familia, c) = letra(f, classe, cores);
                (tamanho, bits, cor) = (t.unwrap_or(tamanho), familia.unwrap_or(classe.familia) | b, c.unwrap_or(cor));
            }
        }
        let (mut antes, mut depois) = match divisao {
            "chapter" => (50.0, 40.0),
            "section" => (3.5 * ex, 2.3 * ex),
            _ => (3.25 * ex, 1.5 * ex),
        };
        // `\titlespacing*{\chapter}{recuo}{antes}{depois}`.
        for espaco in [format!("\\titlespacing*{{\\{divisao}}}"), format!("\\titlespacing{{\\{divisao}}}")] {
            if let Some(i) = preambulo.find(&espaco) {
                let resto = &preambulo[i + espaco.len()..];
                let Some((_, a)) = grupo(resto) else { continue };
                let Some((v1, b)) = grupo(&resto[a..]) else { continue };
                antes = dimensao(v1, classe).unwrap_or(antes);
                depois = grupo(&resto[a + b..]).and_then(|(v2, _)| dimensao(v2, classe)).unwrap_or(depois);
            }
        }
        saida[nivel] = format!("{tamanho} {bits} {cor:08x} {antes} {depois}");
    }
    saida
}

/// "Silva", "Silva e Souza" (na ABNT, "SILVA; SOUZA"), "Silva et al.".
fn autores_no_texto(autores: &[featherdocs_core::modelo::Autor], entre: &str) -> String {
    match autores {
        [] => String::new(),
        [a] => a.sobrenome.clone(),
        [a, b] => format!("{}{entre}{}", a.sobrenome, b.sobrenome),
        [a, ..] => format!("{} et al.", a.sobrenome),
    }
}

/// O número de cada `\label`, contado como o LaTeX conta: o da última seção, figura, tabela ou
/// equação numerada aberta antes dele.
/// Os links como o `hyperref` os pinta com `colorlinks`: cada tipo na sua cor (`linkcolor`, `citecolor`,
/// `urlcolor`) e sem sublinhado. O `\textcolor` escrito à mão fica.
fn cores_dos_links(doc: &mut Documento, preambulo: &str, cores: &HashMap<String, u32>) {
    let mut opcoes = String::new();
    if let Some(p) = preambulo.find("{hyperref}") {
        if let Some(a) = preambulo[..p].rfind("\\usepackage") {
            opcoes.push_str(opcional(&preambulo[a + 11..p]).map_or("", |(o, _)| o));
        }
    }
    for (i, _) in preambulo.match_indices("\\PassOptionsToPackage") {
        if let Some((g, n)) = grupo(&preambulo[i + 21..]) {
            if preambulo[i + 21 + n..].trim_start().starts_with("{hyperref}") {
                opcoes.push(',');
                opcoes.push_str(g);
            }
        }
    }
    for (i, _) in preambulo.match_indices("\\hypersetup") {
        if let Some((g, _)) = grupo(&preambulo[i + 11..]) {
            opcoes.push(',');
            opcoes.push_str(g);
        }
    }
    let valor = |chave: &str| {
        opcoes.split(',').filter_map(|o| o.split_once('=').map(|(k, v)| (k.trim(), v.trim())).or(Some((o.trim(), "true")))).filter(|(k, _)| *k == chave).last().map(|(_, v)| v.to_string())
    };
    // Sem `colorlinks` (o `pdfborder={0 0 0}` da exportação, ou o `hyperref` puro, que só põe moldura na
    // tela) o link fica com a cor padrão: assim o código exportado volta igual.
    if !valor("colorlinks").is_some_and(|v| v != "false") {
        // O `hidelinks` escrito no documento deixa o link igual ao texto: preto e sem sublinhado.
        if valor("hidelinks").is_some() {
            for t in doc.blocos.iter_mut().flat_map(|b| b.conteudo.textos_mut()).flatten() {
                if t.link.is_some() && t.fmt.cor == 0 {
                    t.fmt.cor = 0x000000FF;
                }
            }
        }
        return;
    }
    let cor = |chave: &str, padrao: &str| valor(chave).and_then(|v| resolver_cor(cores, &v)).or_else(|| resolver_cor(cores, padrao)).unwrap_or(0x000000FF);
    let (interno, citacao, endereco) = (cor("linkcolor", "red"), cor("citecolor", "green"), cor("urlcolor", "magenta"));
    for t in doc.blocos.iter_mut().flat_map(|b| b.conteudo.textos_mut()).flatten() {
        if t.fmt.cor != 0 {
            continue;
        }
        t.fmt.cor = match (&t.marca, t.link.as_deref()) {
            (Some(m), _) if matches!(**m, Marca::Citacao { .. }) => citacao,
            (_, Some(l)) if l.starts_with("http") || l.starts_with("mailto:") => endereco,
            (_, Some(_)) => interno,
            _ => continue,
        };
    }
}

/// O número que cada `\label` mostra e o nome do que ele numera ("seção", "figura", "definição"),
/// e os números dos teoremas na ordem em que aparecem.
fn numeros_dos_rotulos(corpo: &str, teoremas: &HashMap<String, pacotes::Teorema>) -> (HashMap<String, (String, String)>, std::collections::VecDeque<String>) {
    let mut saida = HashMap::new();
    let mut fila = std::collections::VecDeque::new();
    let (mut secoes, mut figuras, mut tabelas, mut equacoes, mut listagens, mut sub) = ([0u32; 4], 0u32, 0u32, 0u32, 0u32, 0u8);
    let mut apendice = false;
    // Contador de teorema → (seção em que contou por último, número).
    let mut contadores: HashMap<String, (u32, u32)> = HashMap::new();
    let mut atual = (String::new(), String::new());
    let mut i = 0;
    while let Some(k) = corpo[i..].find('\\') {
        i += k;
        let (nome, n) = comando(&corpo[i..]);
        let depois = &corpo[i + n..];
        let nivel = ["section", "subsection", "subsubsection"].iter().position(|d| *d == nome);
        let ambiente = grupo(depois).map(|(g, _)| g.trim());
        if nome == "appendix" {
            apendice = true;
            secoes.fill(0);
        } else if let Some(nv) = nivel.filter(|_| !depois.starts_with('*')) {
            secoes[nv] += 1;
            secoes[nv + 1..].fill(0);
            let mut partes: Vec<String> = secoes[..=nv].iter().map(u32::to_string).collect();
            if apendice {
                partes[0] = featherdocs_core::paginacao::letra(secoes[0]);
            }
            atual = (partes.join("."), "seção".into());
        } else if nome == "begin" {
            match ambiente {
                Some("figure" | "figure*") => {
                    figuras += 1;
                    sub = 0;
                    atual = (figuras.to_string(), "figura".into());
                }
                Some("subfigure") => {
                    atual = (format!("{figuras}{}", char::from(b'a' + sub)), "figura".into());
                    sub += 1;
                }
                Some("table" | "table*" | "longtable") => {
                    tabelas += 1;
                    atual = (tabelas.to_string(), "tabela".into());
                }
                Some("equation" | "align" | "gather" | "multline") => {
                    equacoes += 1;
                    atual = (equacoes.to_string(), "equação".into());
                }
                // O `label=` da listagem vem nas opções.
                Some("lstlisting") => {
                    let opcoes = opcional(&depois[grupo(depois).map_or(0, |g| g.1)..]).map_or("", |o| o.0);
                    if opcoes.contains("caption") {
                        listagens += 1;
                        if let Some((_, r)) = opcoes_chave_valor(opcoes).into_iter().find(|(k, _)| k == "label") {
                            saida.entry(r.trim_matches(['{', '}']).trim().to_string()).or_insert((listagens.to_string(), "listagem".to_string()));
                        }
                    }
                }
                Some(t) if teoremas.contains_key(t) => {
                    let t = &teoremas[t];
                    if t.numerado {
                        let c = contadores.entry(t.contador.clone()).or_insert((secoes[0], 0));
                        if t.por_secao && c.0 != secoes[0] {
                            *c = (secoes[0], 0);
                        }
                        c.1 += 1;
                        let numero = if t.por_secao { format!("{}.{}", secoes[0], c.1) } else { c.1.to_string() };
                        fila.push_back(numero.clone());
                        atual = (numero, t.nome.to_lowercase());
                    }
                }
                _ => {}
            }
        } else if nome == "end" && ambiente == Some("subfigure") {
            atual = (figuras.to_string(), "figura".into());
        } else if nome == "label" {
            if let Some((g, _)) = grupo(depois) {
                saida.entry(g.trim().to_string()).or_insert_with(|| atual.clone());
            }
        }
        i += n.max(1);
    }
    (saida, fila)
}

/// "seção 3" → "Seção 3".
fn maiuscula_inicial(t: &str) -> String {
    let mut c = t.chars();
    c.next().map_or(String::new(), |p| p.to_uppercase().chain(c).collect())
}

/// A entrelinha do LaTeX (`\baselineskip` = 1,2 × o corpo) no múltiplo do FeatherDocs (1,4 × o corpo = 1,15).
/// A entrelinha é a de uma classe do LaTeX (corpo 10, 11 ou 12): na exportação não precisa de `\baselineskip`.
pub(crate) fn e_entrelinha_latex(v: f32) -> bool {
    [TAMANHOS_10PT, TAMANHOS_11PT, TAMANHOS_12PT].into_iter().any(|tamanhos| (Classe { tamanhos, ..Classe::default() }.entrelinha() - v).abs() < 0.01)
}

struct Analisador<'a> {
    s: &'a str,
    i: usize,
    ctx: Ctx,
    saida: Vec<Bloco>,
    /// O `\parindent` dos parágrafos do corpo (0 dentro de ambientes) e se o primeiro depois do
    /// título também recua (`indentfirst`).
    recuo: (f32, bool),
    /// O ambiente que está sendo lido começou parágrafo (ver `ambiente`).
    vertical: bool,
}

/// Níveis das divisões: `(comando, nível sem \chapter, nível com \chapter)`.
const DIVISOES: [(&str, u8, u8); 6] = [("chapter", 1, 1), ("section", 1, 2), ("subsection", 2, 3), ("subsubsection", 3, 4), ("paragraph", 4, 5), ("subparagraph", 5, 6)];

/// Comandos que o `\renewcommand` muda e que são parâmetros, não macros de texto.
const PARAMETROS: &[&str] = &["arraystretch", "baselinestretch", "bibname", "refname", "headrulewidth", "footrulewidth", "familydefault", "contentsname", "figurename", "tablename", "listfigurename", "listtablename", "thefootnote"];

/// Dos [`IGNORADOS`], os que mudariam o resultado e o FeatherDocs não reproduz (vão para o aviso);
/// os outros já foram lidos noutro lugar (`\pagestyle`, `\linenumbers`) ou não mudam nada.
const SEM_EFEITO: &[&str] = &["captionsetup", "arrayrulecolor", "renewcommand", "pagenumbering", "setlist", "dimexpr"];

/// Comandos que somem sozinhos, com quantos argumentos `{…}` levam junto.
const IGNORADOS: &[(&str, usize)] = &[
    ("documentclass", 1),
    ("usepackage", 1),
    ("label", 1),
    ("setlength", 2),
    ("pagestyle", 1),
    ("thispagestyle", 1),
    ("bibliographystyle", 1),
    ("bibliography", 1),
    ("noindent", 0),
    ("indent", 0),
    ("bigskip", 0),
    ("medskip", 0),
    ("smallskip", 0),
    ("par", 0),
    ("newline", 0),
    ("vfill", 0),
    ("relax", 0),
    ("clearpage", 0),
    ("pagebreak", 0),
    ("setlist", 1),
    ("dimexpr", 0),
    ("arrayrulecolor", 1),
    ("microtype", 0),
    ("pagenumbering", 1),
    ("captionsetup", 1),
    ("fancyhf", 1),
    ("selectfont", 0),
    // No corpo, `\renewcommand{\arraystretch}{1.5}` só muda a arrumação (as macros já foram lidas antes).
    ("renewcommand", 2),
    ("citebrackets", 0),
    ("linenumbers", 0),
    ("pagewiselinenumbers", 0),
    ("pagecolor", 1),
    ("hypersetup", 1),
    ("newlength", 1),
    ("addtolength", 2),
    ("settowidth", 2),
    ("crefname", 3),
    ("Crefname", 3),
    ("theoremstyle", 1),
    ("qedhere", 0),
    ("lstset", 1),
    ("sisetup", 1),
];

impl<'a> Analisador<'a> {
    /// O `\topsep` da classe (8, 9 ou 10 pt): o espaço em volta de listas e teoremas.
    fn topsep(&self) -> f32 {
        (self.ctx.classe.tamanhos[4] * 0.8).round()
    }

    /// O vão entre os itens de uma lista de `nivel` (`\itemsep` + `\parsep`): o do preâmbulo no nível
    /// de fora; dentro, o da classe (`\@listii` 2 + 2 pt, `\@listiii` 2 + 0), a não ser que o
    /// `\setlist` tenha mudado o vão.
    fn vao_entre_itens(&self, ordenado: bool, nivel: u8) -> f32 {
        let vao = self.ctx.itemsep[usize::from(ordenado)];
        let corpo = self.ctx.classe.tamanhos[4];
        let padrao = 2.0 * if corpo >= 12.0 { 5.0 } else if corpo > 10.0 { 4.5 } else { 4.0 };
        let grande = corpo >= 12.0;
        match nivel {
            0 => vao,
            _ if (vao - padrao).abs() > 0.01 => vao,
            1 => if grande { 5.0 } else { 4.0 },
            _ => if grande { 2.5 } else { 2.0 },
        }
    }

    /// Espaço antes do bloco `de` e depois do último lido (o maior, se já tinham um).
    fn espaco_em_volta(&mut self, de: usize, espaco: f32) {
        if let Some(b) = self.saida.get_mut(de) {
            b.atrib.antes = Some(b.atrib.antes.unwrap_or(0.0).max(espaco));
        }
        if de >= self.saida.len() {
            return;
        }
        if let Some(b) = self.saida.last_mut() {
            b.atrib.depois = Some(b.atrib.depois.unwrap_or(0.0).max(espaco));
        }
    }

    fn novo(s: &'a str, ctx: Ctx) -> Self {
        Self { s, i: 0, ctx, saida: Vec::new(), recuo: (0.0, false), vertical: false }
    }

    /// Prende o `\label{nome}` ao último bloco lido (o título logo antes, a imagem da figura).
    /// shortcut: o rótulo antes de qualquer bloco (no começo de um ambiente) é perdido.
    fn rotular(&mut self, nome: String) {
        let Some(b) = self.saida.last_mut() else { return };
        if b.id & avisos::MARCA_DE_ROTULO != 0 {
            let indice = (b.id & !avisos::MARCA_DE_ROTULO) as usize;
            if let Some(v) = self.ctx.avisos.0.borrow_mut().rotulos.get_mut(indice) {
                v.push(nome);
            }
        } else {
            b.id = self.ctx.avisos.rotular(vec![nome]);
        }
    }

    fn push_bloco(&mut self, c: Conteudo) {
        let atrib = self.atrib_para(&c);
        self.saida.push(Bloco { id: 0, conteudo: c, atrib, mesclagens: Vec::new(), latex: String::new(), tabela: Default::default(), imagem: Default::default() });
    }

    /// Os atributos que o contexto dá ao bloco: fundo, alinhamento, entrelinha, espaços
    /// (`\parskip`, `itemsep`, o `\vspace` em espera), cor do marcador e da régua.
    fn atrib_para(&mut self, c: &Conteudo) -> Atrib {
        let mut a = Atrib { fundo: self.ctx.fundo, alinhamento: self.ctx.alinhamento, ..Atrib::default() };
        let texto = matches!(c, Conteudo::Titulo { .. } | Conteudo::Paragrafo { .. } | Conteudo::Item { .. } | Conteudo::Citacao { .. });
        if texto {
            a.entrelinha = self.ctx.classe.entrelinha() * if self.ctx.espaco > 0.0 { self.ctx.espaco } else { 1.0 };
        }
        match c {
            Conteudo::Tabela { .. } => {
                if self.ctx.estica_tabela > 0.0 {
                    a.entrelinha = 1.15 * self.ctx.estica_tabela;
                }
                // Na tabela, o fundo pinta uma linha sim, outra não; `marcado` diz se são as pares.
                if let Some((cor, pares)) = self.ctx.zebra {
                    (a.fundo, a.marcado) = (cor, pares);
                }
            }
            Conteudo::Paragrafo { .. } => a.depois = self.ctx.parskip,
            Conteudo::Item { ordenado, nivel, .. } => {
                a.depois = Some(self.vao_entre_itens(*ordenado, *nivel));
                a.cor_marcador = self.ctx.cor_marcador[usize::from(matches!(c, Conteudo::Item { ordenado: true, .. }))];
            }
            Conteudo::Regua => {
                a.cor_borda = self.ctx.estado.cor;
                a.antes = Some(0.0);
                a.depois = Some(0.0);
            }
            _ => {}
        }
        a.encher = std::mem::take(&mut self.ctx.encher);
        // O fim da lista ou do `center` (o `\addvspace`) soma ao `\vspace` que vem logo depois; sem ele,
        // vale o maior, como já faz o "depois" do último bloco.
        let fim = std::mem::take(&mut self.ctx.fim_de_ambiente);
        if self.ctx.vspace != 0.0 && (texto || matches!(c, Conteudo::Regua)) {
            a.antes = Some((self.ctx.vspace + fim).max(0.0));
            self.ctx.vspace = 0.0;
        }
        self.ctx.vspace_em_linha = 0.0;
        a
    }

    /// Texto em linha com as declarações em vigor, que ficam como o texto as deixou.
    fn inline_aqui(&mut self, src: &str) -> Vec<Trecho> {
        let mut e = self.ctx.estado;
        let t = inline_em(src, &self.ctx, &mut e);
        self.ctx.estado = e;
        t
    }

    fn resto(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn executar(mut self) -> Vec<Bloco> {
        loop {
            let n = self.resto().len() - self.resto().trim_start().len();
            self.i += n;
            if self.i >= self.s.len() {
                break;
            }
            if !self.bloco() {
                self.paragrafo();
            }
        }
        // O espaço pedido no fim (o `\\[8ex]` antes do `\end{center}`) fica embaixo do último bloco.
        if self.ctx.vspace > 0.0 {
            if let Some(b) = self.saida.last_mut() {
                b.atrib.depois = Some(b.atrib.depois.unwrap_or(0.0) + self.ctx.vspace);
            }
        }
        self.saida
    }

    /// Tenta tratar um bloco que começa aqui; `false` se é texto corrido.
    fn bloco(&mut self) -> bool {
        let r = self.resto();
        if r.starts_with("$$") {
            return self.equacao_de("$$", "$$");
        }
        if r.starts_with('{') {
            return self.grupo_com_blocos();
        }
        if !r.starts_with('\\') {
            return false;
        }
        // `\fbox{\begin{minipage}…}`: a moldura preta em volta de blocos (em linha, só o texto sai).
        if r.starts_with("\\fbox") && grupo(&r[5..]).is_some_and(|(g, _)| g.contains("\\begin{minipage}") || g.contains("\\par")) {
            if let Some(feito) = self.caixa_colorida(None) {
                return feito;
            }
        }
        if r.starts_with("\\colorbox") || r.starts_with("\\fcolorbox") {
            if let Some(feito) = self.caixa_colorida(Some(r.starts_with("\\f"))) {
                return feito;
            }
        }
        let (nome, n) = comando(r);
        if nome == "[" {
            return self.equacao_de("\\[", "\\]");
        }
        if let Some(feito) = self.declaracao_de_bloco(nome, n) {
            return feito;
        }
        if let Some((_, a, b)) = DIVISOES.iter().find(|d| d.0 == nome) {
            return self.divisao(n, if self.ctx.capitulo { *b } else { *a });
        }
        match nome {
            "begin" => self.ambiente(n),
            "maketitle" => {
                self.i += n;
                self.titulo_do_documento();
                true
            }
            // Numa página que acabou de começar a quebra não faz nada (o `\clearpage` depois da capa).
            "newpage" | "clearpage" | "pagebreak" if self.saida.last().is_none_or(|b| matches!(b.conteudo, Conteudo::QuebraDePagina)) => {
                self.i += n;
                true
            }
            "newpage" | "clearpage" | "pagebreak" => self.simples(n, Conteudo::QuebraDePagina),
            "printbibliography" | "bibliography" | "printindex" | "nocite" => self.referencias(nome, n),
            "tableofcontents" => self.simples(n, Conteudo::Sumario { niveis: 3, sem_numeros: false, sem_pontilhado: false, lista: 0, latex: true }),
            "listoffigures" => self.simples(n, Conteudo::Sumario { niveis: 1, sem_numeros: false, sem_pontilhado: false, lista: 1, latex: true }),
            "listoftables" => self.simples(n, Conteudo::Sumario { niveis: 1, sem_numeros: false, sem_pontilhado: false, lista: 2, latex: true }),
            "hrule" => {
                self.i += n;
                let r = self.resto();
                let mut skip = 0;
                for (idx, c) in r.char_indices() {
                    if c == '\n' || c == '}' || c == '\\' {
                        break;
                    }
                    skip = idx + c.len_utf8();
                }
                self.i += skip;
                self.push_bloco(Conteudo::Regua);
                true
            }
            "includegraphics" => self.imagem_solta(n),
            "caption" | "captionof" => self.legenda_solta(n),
            "label" => {
                self.i += n;
                if let Some((g, k)) = grupo(self.resto()) {
                    self.i += k;
                    self.rotular(g.trim().to_string());
                }
                true
            }
            "title" | "author" | "date" => self.guardar_meta(nome, n),
            _ => self.ignorado(nome, n),
        }
    }

    /// `{…}` que tem bloco dentro (`{\color{x}\hrule}`): é um grupo, o que ele declara não vaza.
    /// Sem bloco dentro, é texto (`false`).
    fn grupo_com_blocos(&mut self) -> bool {
        let Some((dentro, k)) = grupo(self.resto()) else { return false };
        // `{\Huge Título \par}` (o título de uma capa) também é um parágrafo só dele.
        if !tem_bloco(dentro) && !dentro.trim_end().ends_with("\\par") {
            return false;
        }
        let filhos = Analisador::novo(dentro, self.ctx.clone()).executar();
        // O espaço pendente (`\vspace`, `\\[7cm]`) foi para o primeiro bloco de dentro.
        if !filhos.is_empty() {
            self.ctx.vspace = 0.0;
        }
        self.saida.extend(filhos);
        self.i += k;
        true
    }

    /// `\colorbox[modelo]{fundo}{…}` e `\fcolorbox[modelo]{moldura}{fundo}{…}` com blocos dentro (o
    /// `\parbox` de dentro é só o que ele guarda); `None` se falta argumento.
    fn caixa_colorida(&mut self, moldura: Option<bool>) -> Option<bool> {
        let r = self.resto();
        let fbox = moldura.is_none();
        let moldura = moldura.unwrap_or(false);
        let mut p = if fbox { 5 } else if moldura { 10 } else { 9 };
        let modelo = opcional(&r[p..]).map(|(o, k)| {
            p += k;
            o
        });
        let cor = |nome: &str| match modelo {
            Some(m) => parsear_cor(m, nome),
            None => resolver_cor(&self.ctx.cores, nome),
        };
        let mut cor_moldura = 0;
        if moldura {
            let (c, k) = grupo(&r[p..])?;
            cor_moldura = cor(c).unwrap_or(0x000000FF);
            p += k;
        }
        let (cor_nome, k1) = if fbox { ("", 0) } else { grupo(&r[p..])? };
        if fbox {
            cor_moldura = 0x000000FF;
        }
        p += k1;
        let (mut conteudo, k2) = grupo(&r[p..])?;
        // A `minipage` de dentro só dá a largura: o conteúdo é o que importa.
        if let Some(dentro) = conteudo.trim().strip_prefix("\\begin{minipage}") {
            let mut k = 0;
            while let Some((_, n)) = opcional(&dentro[k..]) {
                k += n;
            }
            k += grupo(&dentro[k..]).map_or(0, |g| g.1);
            conteudo = dentro[k..].trim_end().strip_suffix("\\end{minipage}").unwrap_or(&dentro[k..]);
        }
        let fundo = cor(cor_nome).unwrap_or(0);
        self.i += p + k2;
        if let Some(dentro) = conteudo.trim().strip_prefix("\\parbox") {
            let mut k = 0;
            while let Some((_, n)) = opcional(&dentro[k..]) {
                k += n;
            }
            k += grupo(&dentro[k..]).map_or(0, |g| g.1);
            conteudo = grupo(&dentro[k..]).map_or(conteudo, |g| g.0);
        }
        let mut ctx = self.ctx.clone();
        ctx.fundo = fundo;
        if conteudo.contains("\\centering") {
            ctx.alinhamento = Alinhamento::Centro;
        }
        let filhos = Analisador::novo(conteudo, ctx).executar();
        self.caixa_de_blocos(filhos, fundo, cor_moldura);
        Some(true)
    }

    /// `\vspace`, `\centering`, `\raggedright`, `\raggedleft` entre blocos.
    fn declaracao_de_bloco(&mut self, nome: &str, n: usize) -> Option<bool> {
        match nome {
            "vspace" | "vspace*" => {
                let resto = &self.resto()[n..];
                let (g, k) = grupo(resto)?;
                if let Some(d) = dimensao(g, self.ctx.classe) {
                    self.ctx.vspace += d;
                }
                self.ctx.encher |= matches!(g.trim(), "\\fill" | "\\vfill" | "\\stretch{1}");
                self.i += n + k;
            }
            "vfill" | "vfil" => {
                self.ctx.encher = true;
                self.i += n;
            }
            // `\\bigskipamount` e companhia (12, 6 e 3 pt); no fim do parágrafo, o espaço é depois dele.
            "bigskip" | "medskip" | "smallskip" => {
                let d = match nome {
                    "bigskip" => 12.0,
                    "medskip" => 6.0,
                    _ => 3.0,
                };
                self.ctx.vspace += d;
                let antes = &self.s[..self.i];
                if !self.saida.is_empty() && antes[antes.trim_end().len()..].matches('\n').count() < 2 {
                    self.ctx.vspace_em_linha += d;
                }
                self.i += n;
            }
            "centering" => {
                self.ctx.alinhamento = Alinhamento::Centro;
                self.i += n;
            }
            "onehalfspacing" | "doublespacing" | "singlespacing" | "setstretch" | "linespread" => {
                let k = if matches!(nome, "setstretch" | "linespread") { grupo(&self.resto()[n..]).map_or(0, |g| g.1) } else { 0 };
                self.ctx.espaco = espaco_do(&self.resto()[..n + k]);
                self.i += n + k;
            }
            // `\rowcolors{2}{a}{b}`: da linha 2 em diante, as ímpares com `a` e as pares com `b`.
            // shortcut: uma cor só (a que não é branca), contada da segunda linha; duas cores pedem o motor.
            "rowcolors" => {
                let r = &self.resto()[n..];
                let k0 = opcional(r).map_or(0, |o| o.1);
                let (_, a) = grupo(&r[k0..])?;
                let (impar, b) = grupo(&r[k0 + a..])?;
                let (par, c) = grupo(&r[k0 + a + b..])?;
                let cor = |nome: &str| resolver_cor(&self.ctx.cores, nome).filter(|&c| c != 0xFFFFFFFF && c != 0);
                self.ctx.zebra = cor(impar).map(|c| (c, false)).or_else(|| cor(par).map(|c| (c, true)));
                self.i += n + k0 + a + b + c;
            }
            "raggedright" => {
                self.ctx.alinhamento = Alinhamento::Esquerda;
                self.i += n;
            }
            "raggedleft" => {
                self.ctx.alinhamento = Alinhamento::Direita;
                self.i += n;
            }
            _ => return None,
        }
        Some(true)
    }

    fn simples(&mut self, n: usize, c: Conteudo) -> bool {
        self.i += n;
        self.push_bloco(c);
        true
    }

    /// `\printbibliography[…]` (ou `\bibliography{…}`) e `\printindex` viram as listas geradas;
    /// o `\nocite{…}` só some (o `\nocite{*}` liga "todas" depois, em `ler`).
    fn referencias(&mut self, nome: &str, n: usize) -> bool {
        self.i += n;
        let mut opcoes = String::new();
        while let Some((o, k)) = opcional(self.resto()) {
            opcoes.push_str(o);
            self.i += k;
        }
        if nome != "printbibliography" && nome != "printindex" {
            if let Some((_, k)) = grupo(self.resto()) {
                self.i += k;
            }
        }
        match nome {
            "printindex" => self.push_bloco(Conteudo::IndiceRemissivo),
            "nocite" => {}
            _ => {
                if let Some(titulo) = titulo_da_bibliografia(&opcoes) {
                    let mut ctx = self.ctx.clone();
                    ctx.classe.corpo = 0.0;
                    // O título fica no tamanho do estilo dele (a escala da letra não o troca pelo do corpo).
                    ctx.classe.escala = 1.0;
                    let texto = inline_em(&titulo, &ctx, &mut Estado::default());
                    self.push_bloco(Conteudo::Titulo { nivel: 1, texto });
                }
                self.push_bloco(Conteudo::Bibliografia { todas: false });
                // Só as fontes com a palavra-chave (`keyword=basica`, o `keywords` do `.bib`).
                if let Some((_, k)) = opcoes_chave_valor(&opcoes).into_iter().find(|(c, _)| c == "keyword") {
                    if let Some(b) = self.saida.last_mut() {
                        b.latex = format!("keyword={}", k.trim().trim_matches(['{', '}']));
                    }
                }
                self.rotular("@referencias".into());
            }
        }
        true
    }

    fn ignorado(&mut self, nome: &str, n: usize) -> bool {
        let Some((_, args)) = IGNORADOS.iter().find(|d| d.0 == nome) else { return false };
        // `\pagenumbering{arabic}` é o que o FeatherDocs já faz; os outros (roman…) não.
        let mut feito = nome == "pagenumbering" && grupo(&self.resto()[n..]).is_some_and(|(g, _)| g.trim() == "arabic");
        if nome == "renewcommand" {
            let r = &self.resto()[n..];
            if let Some((alvo, k)) = grupo(r) {
                let valor = grupo(&r[k..]).map_or("", |g| g.0).trim();
                match alvo.trim() {
                    // Vale para as tabelas deste grupo (o ambiente `table`), como no LaTeX.
                    "\\arraystretch" => {
                        self.ctx.estica_tabela = valor.parse().unwrap_or(0.0);
                        feito = true;
                    }
                    // Sem título na lista: ela já vem logo depois do título escrito pela pessoa.
                    "\\bibname" | "\\refname" => feito = valor.is_empty(),
                    _ => {}
                }
            }
        }
        if SEM_EFEITO.contains(&nome) && !feito {
            self.ctx.avisos.anotar(TipoDeAviso::SemEfeito, &format!("\\{nome}"));
        }
        self.i += n;
        while let Some((_, k)) = opcional(self.resto()) {
            self.i += k;
        }
        for _ in 0..*args {
            if let Some((_, k)) = grupo(self.resto()) {
                self.i += k;
            }
        }
        true
    }

    fn guardar_meta(&mut self, nome: &str, n: usize) -> bool {
        let Some((g, k)) = grupo(&self.resto()[n..]) else { return false };
        let t = texto_puro(&inline(g, &self.ctx.cores)).trim().to_string();
        match nome {
            "title" => (self.ctx.titulo, self.ctx.titulo_fonte) = (t, g.to_string()),
            "author" => (self.ctx.autor, self.ctx.autor_fonte) = (t, g.to_string()),
            _ => self.ctx.data_fonte = Some(g.to_string()),
        }
        self.i += n + k;
        true
    }

    /// O `\maketitle` da classe `article`: título em `\LARGE`, autor e data em `\large`, centrados
    /// (o `\thanks` vira nota de rodapé; sem `\date`, a data de hoje).
    fn titulo_do_documento(&mut self) {
        let data = self.ctx.data_fonte.clone().unwrap_or_else(|| "\\today".into());
        // Os `\vskip` do `\maketitle`: 1,5 em entre título e autor, 1 em entre autor e data.
        let em = self.ctx.classe.tamanhos[4];
        for (fonte, tamanho, antes) in [(self.ctx.titulo_fonte.clone(), "LARGE", 0.0), (self.ctx.autor_fonte.clone(), "large", 1.5 * em), (data, "large", em)] {
            let mut ctx = self.ctx.clone();
            ctx.estado.tamanho = ctx.classe.tamanho_de(tamanho).unwrap_or(0.0);
            // Vários autores (`\and`) ficam lado a lado, separados.
            let fonte = fonte.replace("\\and", "\\quad\\quad ");
            let texto = inline_em(&fonte, &ctx, &mut ctx.estado.clone());
            if texto.is_empty() {
                continue;
            }
            self.push_bloco(Conteudo::Paragrafo { texto });
            if let Some(b) = self.saida.last_mut() {
                b.atrib.alinhamento = Alinhamento::Centro;
                b.atrib.antes = Some(antes);
            }
        }
        self.ctx.vspace += 1.5 * em;
    }

    fn divisao(&mut self, n: usize, nivel: u8) -> bool {
        let mut p = n;
        if let Some((_, k)) = opcional(&self.resto()[p..]) {
            p += k;
        }
        let Some((g, k)) = grupo(&self.resto()[p..]) else { return false };
        let mut ctx = self.ctx.clone();
        ctx.classe.corpo = 0.0;
        // O título fica no tamanho do estilo dele (a escala da letra não o troca pelo do corpo).
        ctx.classe.escala = 1.0;
        let texto = inline_em(g, &ctx, &mut Estado::default());
        self.push_bloco(Conteudo::Titulo { nivel, texto });
        self.i += p + k;
        true
    }

    fn equacao_de(&mut self, abre: &str, fecha: &str) -> bool {
        let corpo = &self.resto()[abre.len()..];
        let Some(f) = corpo.find(fecha) else { return false };
        self.push_bloco(Conteudo::Equacao { latex: unidades::em_formula(&limpar_formula(&corpo[..f])) });
        self.i += abre.len() + f + fecha.len();
        true
    }

    fn imagem_solta(&mut self, n: usize) -> bool {
        let mut p = n;
        let mut largura = 0.0;
        if let Some((o, k)) = opcional(&self.resto()[p..]) {
            largura = largura_da_imagem(o, &self.ctx);
            p += k;
        }
        let Some((g, k)) = grupo(&self.resto()[p..]) else { return false };
        self.push_bloco(Conteudo::Imagem { origem: g.trim().to_string(), legenda: Vec::new(), largura, altura: 0.0 });
        self.i += p + k;
        true
    }

    /// `\caption{…}` fora da figura (na tabela) ou `\captionof{figure|table}{…}`: a legenda numerada.
    fn legenda_solta(&mut self, n: usize) -> bool {
        let mut p = n;
        let mut tipo = if self.ctx.em_figura { 1 } else { 2 };
        if self.resto()[..n].ends_with("captionof") {
            let Some((t, k)) = grupo(&self.resto()[p..]) else { return false };
            tipo = if t.trim() == "figure" { 1 } else { 2 };
            p += k;
        }
        if let Some((_, k)) = opcional(&self.resto()[p..]) {
            p += k;
        }
        let Some((g, k)) = grupo(&self.resto()[p..]) else { return false };
        let mut texto = self.inline_aqui(g);
        self.i += p + k;
        // A legenda da subfigura: "(a) …", sem número próprio, embaixo da imagem dela.
        if let Some(letra) = self.ctx.subfigura {
            texto.insert(0, Trecho { texto: format!("({letra}) "), estilo: self.ctx.classe.familia, ..Trecho::default() });
            match self.saida.last_mut().map(|b| &mut b.conteudo) {
                Some(Conteudo::Imagem { legenda, .. }) if legenda.is_empty() => *legenda = texto,
                _ => self.push_bloco(Conteudo::Paragrafo { texto }),
            }
            return true;
        }
        if self.ctx.legenda_negrito {
            texto.iter_mut().for_each(|t| t.estilo |= NEGRITO);
        }
        self.push_legenda(texto, tipo);
        true
    }

    /// O parágrafo de legenda (1 figura, 2 tabela), centrado: o número sai sozinho ("Tabela 1 – ").
    fn push_legenda(&mut self, texto: Vec<Trecho>, tipo: u8) {
        self.push_bloco(Conteudo::Paragrafo { texto });
        if let Some(b) = self.saida.last_mut() {
            b.atrib.legenda = tipo;
            b.atrib.alinhamento = Alinhamento::Centro;
            b.latex = self.ctx.separador_legenda.to_string();
        }
    }

    /// Texto corrido até uma linha em branco, o começo de outro bloco ou o fim de parágrafo
    /// (`\par`, `\\`, `\newline`) fora de grupo.
    fn paragrafo(&mut self) {
        let inicio = self.i;
        let (fim, proximo) = self.fim_do_paragrafo(inicio);
        let texto = self.inline_aqui(&self.s[inicio..fim]);
        // O `\\[1ex]` que fechou o parágrafo: espaço antes do próximo.
        let espaco_depois = self.s[fim..proximo].strip_prefix("\\\\").and_then(opcional).and_then(|(d, _)| dimensao(d, self.ctx.classe));
        self.i = proximo;
        // O `\hfill` sozinho entre duas `minipage`/`subfigure` não é um parágrafo.
        let so_espaco = texto.iter().all(|t| t.marca.is_none() && t.texto.chars().all(|c| c == '\t' || c == ' '));
        if !texto.is_empty() && !so_espaco {
            self.push_bloco(Conteudo::Paragrafo { texto });
            // O LaTeX recua o parágrafo novo (depois de linha em branco), não o que segue um título ou
            // um ambiente na mesma linha, nem o que pede `\noindent`.
            let antes = &self.s[..inicio];
            let novo = antes.trim_end_matches([' ', '\t']).is_empty() || antes[antes.trim_end().len()..].matches('\n').count() >= 2;
            let depois_do_titulo = matches!(self.saida.iter().rev().nth(1).map(|b| &b.conteudo), Some(Conteudo::Titulo { .. }));
            let (recuo, primeiro) = self.recuo;
            if recuo > 0.0 && novo && (primeiro || !depois_do_titulo) && !self.s[inicio..].trim_start().starts_with("\\noindent") && self.ctx.alinhamento != Alinhamento::Centro {
                if let Some(b) = self.saida.last_mut() {
                    b.atrib.primeira = recuo;
                }
            }
        }
        self.ctx.vspace += espaco_depois.unwrap_or(0.0);
    }

    /// Onde o parágrafo que começa em `inicio` termina e onde a leitura continua.
    fn fim_do_paragrafo(&mut self, inicio: usize) -> (usize, usize) {
        let mut j = inicio;
        let mut prof = 0i32;
        while j < self.s.len() {
            let r = &self.s[j..];
            if prof == 0 && j > inicio && (r.starts_with("\n\n") || r.starts_with("\n\r\n") || ComecaBloco::sim(r)) {
                return (j, j);
            }
            // Fórmula em linha: o `\\`, o `&` e o `\begin{pmatrix}` de dentro dela são da fórmula, não do parágrafo.
            if let Some(n) = fim_da_formula_em_linha(r) {
                j += n;
                continue;
            }
            if r.starts_with('\\') {
                let (nome, n) = comando(r);
                // `\\` sem medida é quebra de linha dentro do parágrafo; com medida (`\\[2mm]`) vira espaço entre parágrafos.
                let quebra = matches!(nome, "\\" | "newline" | "linebreak") && !(nome == "\\" && r[n..].starts_with('['));
                if prof == 0 && !quebra && matches!(nome, "\\" | "par") {
                    // O `[2mm]` é lido em `paragrafo`, depois de gravar este: o espaço é do de baixo.
                    let k = n + opcional(&r[n..]).map_or(0, |(_, len)| len);
                    return (j, j + k);
                }
                j += n.max(1);
                continue;
            }
            match r.chars().next() {
                Some('{') => prof += 1,
                Some('}') => prof -= 1,
                _ => {}
            }
            j += r.chars().next().map_or(1, char::len_utf8);
        }
        if j == inicio {
            j = inicio + self.s[inicio..].chars().next().map_or(1, char::len_utf8);
        }
        (j, j)
    }
}

/// Há bloco no nível de cima deste texto (fora de grupos)?
fn tem_bloco(s: &str) -> bool {
    let mut prof = 0;
    let mut j = 0;
    while j < s.len() {
        let r = &s[j..];
        if prof == 0 && ComecaBloco::sim(r) {
            return true;
        }
        if r.starts_with('\\') {
            j += comando(r).1.max(1);
            continue;
        }
        match r.chars().next() {
            // Um grupo dentro do outro (`{\rowcolors… {\centering \begin{tabular}…}}`) também conta.
            Some('{') if prof == 0 && grupo(r).is_some_and(|(d, _)| tem_bloco(d)) => return true,
            Some('{') => prof += 1,
            Some('}') => prof -= 1,
            _ => {}
        }
        j += r.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// O que, no meio do texto, abre um bloco novo.
struct ComecaBloco;

impl ComecaBloco {
    fn sim(r: &str) -> bool {
        if r.starts_with("$$") {
            return true;
        }
        if r.starts_with("\\\\") {
            return false;
        }
        if !r.starts_with('\\') {
            return false;
        }
        let (nome, _) = comando(r);
        DIVISOES.iter().any(|d| d.0 == nome)
            || matches!(
                nome,
                "begin" | "[" | "newpage" | "clearpage" | "pagebreak" | "bigskip" | "medskip" | "smallskip" | "maketitle" | "tableofcontents" | "listoffigures" | "listoftables" | "includegraphics" | "caption" | "hrule" | "end" | "colorbox" | "printbibliography" | "printindex" | "nocite"
            )
    }
}

/// O `title=` do `\printbibliography[…]`, que vira um título sem número (no sumário). `None` = a
/// lista com o título dela, no idioma da interface.
fn titulo_da_bibliografia(opcoes: &str) -> Option<String> {
    opcoes_chave_valor(opcoes).into_iter().find(|(c, _)| c == "title").map(|(_, v)| v.trim().trim_matches(['{', '}']).to_string())
}

/// O corpo sem o `\vfill` (ou `\vfil`) do fim, e se ele estava lá.
fn sem_vfill_no_fim(corpo: &str) -> (&str, bool) {
    let t = corpo.trim_end();
    ["\\vfill", "\\vfil"].iter().find_map(|v| t.strip_suffix(v)).map_or((corpo, false), |c| (c, true))
}

/// Se `r` começa uma fórmula em linha (`$…$`, `$$…$$`, `\(…\)`), quantos bytes ela ocupa até o fechamento.
fn fim_da_formula_em_linha(r: &str) -> Option<usize> {
    let (abre, fecha) = if r.starts_with("$$") {
        ("$$", "$$")
    } else if r.starts_with('$') {
        ("$", "$")
    } else if r.starts_with("\\(") {
        ("\\(", "\\)")
    } else {
        return None;
    };
    r[abre.len()..].find(fecha).map(|f| abre.len() + f + fecha.len())
}

fn limpar_formula(f: &str) -> String {
    let mut s = f.to_string();
    while let Some(i) = s.find("\\label{") {
        let Some((_, k)) = grupo(&s[i + 6..]) else { break };
        s.replace_range(i..i + 6 + k, "");
    }
    s.trim().to_string()
}

// ---------------------------------------------------------------------------
// Ambientes (`\begin{…}` … `\end{…}`)
// ---------------------------------------------------------------------------

/// O interior do ambiente `nome` que abre em `s` (logo depois do `\begin{nome}`) e o total de bytes até depois do `\end`.
fn corpo_do_ambiente<'a>(s: &'a str, nome: &str) -> Option<(&'a str, usize)> {
    let abre = format!("\\begin{{{nome}}}");
    let fecha = format!("\\end{{{nome}}}");
    let mut prof = 1;
    let mut j = 0;
    while j < s.len() {
        let r = &s[j..];
        if r.starts_with(&abre) {
            prof += 1;
            j += abre.len();
        } else if r.starts_with(&fecha) {
            prof -= 1;
            if prof == 0 {
                return Some((&s[..j], j + fecha.len()));
            }
            j += fecha.len();
        } else {
            j += r.chars().next().map_or(1, char::len_utf8);
        }
    }
    None
}

impl Analisador<'_> {
    fn ambiente(&mut self, n: usize) -> bool {
        let Some((nome, k)) = grupo(&self.resto()[n..]) else { return false };
        // O ambiente começa parágrafo (depois de linha em branco, de `\par` ou no começo): a lista e o
        // `center` ganham o `\partopsep`.
        let antes = &self.s[..self.i];
        let texto = antes.trim_end();
        self.vertical = antes[texto.len()..].matches('\n').count() >= 2 || texto.ends_with("\\par") || texto.is_empty();
        self.ctx.fim_de_ambiente = 0.0;
        let mut apos = &self.resto()[n + k..];
        let mut extra_p = 0;
        let mut largura = "";
        if nome == "minipage" || nome == "subfigure" {
            while let Some((_, opt_len)) = opcional(&apos[extra_p..]) {
                extra_p += opt_len;
            }
            if let Some((g, grp_len)) = grupo(&apos[extra_p..]) {
                largura = g;
                extra_p += grp_len;
            }
        } else if matches!(nome, "figure" | "figure*" | "table" | "table*") {
            // `[h]`, `[!htpb]`: onde o LaTeX põe o flutuante; aqui ele fica onde está.
            while let Some((_, opt_len)) = opcional(&apos[extra_p..]) {
                extra_p += opt_len;
            }
        } else if nome == "minted" {
            // `\begin{minted}[linenos]{python}`: as opções não são código.
            while let Some((_, opt_len)) = opcional(&apos[extra_p..]) {
                extra_p += opt_len;
            }
        } else if nome == "tabularx" || nome == "tabular*" {
            if let Some((_, grp_len)) = grupo(&apos[extra_p..]) {
                extra_p += grp_len;
            }
        }
        apos = &apos[extra_p..];
        let Some((corpo, total)) = corpo_do_ambiente(apos, nome) else { return false };
        self.i += n + k + extra_p + total;
        let base = nome.trim_end_matches('*');
        let antes = self.saida.len();
        self.ambiente_por_nome(nome, base, largura, corpo);
        // O `\vspace` em espera foi junto para os blocos de dentro (na cópia do contexto) e já foi usado.
        if self.saida.len() > antes {
            self.ctx.vspace = 0.0;
        }
        true
    }

    fn ambiente_por_nome(&mut self, nome: &str, base: &str, largura: &str, corpo: &str) {
        match base {
            "itemize" | "enumerate" => {
                let (de, nivel, vertical) = (self.saida.len(), self.ctx.nivel_lista, self.vertical);
                let topsep = opcional(corpo).and_then(|(o, _)| opcoes_chave_valor(o).into_iter().find(|(k, _)| k == "topsep")).and_then(|(_, v)| dimensao(&v, self.ctx.classe));
                let em_linha = self.ctx.vspace_em_linha;
                self.lista(corpo, base == "enumerate");
                // Em volta: o `\topsep` (+ `\partopsep` se começa parágrafo); a de dentro de um item, o
                // `\topsep` dela mais o `\parsep` do item, que dá o vão entre os itens de fora. O
                // `topsep=` do `enumitem` troca o `\topsep` (o `\parsep` de fora é metade do vão).
                let volta = match (nivel, topsep) {
                    (0, Some(t)) => t,
                    (0, None) => self.topsep() + if vertical { self.ctx.classe.partopsep() } else { 0.0 },
                    (n, Some(t)) => t + self.vao_entre_itens(base == "enumerate", n - 1) / 2.0,
                    (n, None) => self.vao_entre_itens(base == "enumerate", n - 1),
                };
                self.espaco_em_volta(de, volta);
                if em_linha > 0.0 {
                    if let Some(b) = self.saida.get_mut(de) {
                        b.atrib.antes = Some(em_linha + volta);
                    }
                }
                self.ctx.fim_de_ambiente = volta;
            }
            "description" => self.descricao(corpo),
            "verbatim" | "Verbatim" | "lstlisting" | "minted" => self.codigo(corpo, base),
            // O ambiente fica inteiro no LaTeX da equação: o feather-math sabe numerar (`equation`, `align`
            // sem estrela), alinhar no `&` e ler `\label`, `\tag` e `\nonumber`.
            "displaymath" | "math" => self.push_bloco(Conteudo::Equacao { latex: unidades::em_formula(&limpar_formula(corpo)) }),
            "split" => self.push_bloco(Conteudo::Equacao { latex: format!("\\begin{{aligned}}{}\\end{{aligned}}", unidades::em_formula(corpo.trim())) }),
            "equation" | "align" | "eqnarray" | "multline" | "gather" | "flalign" | "alignat" | "xalignat" | "dmath" | "dgroup" | "dseries" | "empheq" => {
                self.push_bloco(Conteudo::Equacao { latex: format!("\\begin{{{nome}}}{}\\end{{{nome}}}", unidades::em_formula(corpo.trim())) });
                // O `\label` de dentro fica na fórmula (o feather-math o lê) e também liga o `\ref` a ela.
                if matches!(base, "equation" | "multline") && corpo.matches("\\label").count() > 1 {
                    let segundo = corpo.match_indices("\\label").nth(1).map_or("", |(p, _)| corpo[p..].split('}').next().unwrap_or(""));
                    self.ctx.avisos.anotar(TipoDeAviso::Erro, &format!("Package amsmath Error: Multiple \\label's: {segundo}}}"));
                }
                let mut r = corpo;
                while let Some(p) = r.find("\\label") {
                    r = &r[p + 6..];
                    if let Some((g, _)) = grupo(r) {
                        self.rotular(g.trim().to_string());
                    }
                }
            }
            // O resumo da classe `article`: "Resumo" em negrito, centrado, e o texto recuado.
            "abstract" => {
                let mut ctx = self.ctx.clone();
                ctx.estado.tamanho = ctx.classe.tamanho_de("small").unwrap_or(0.0);
                let ctx_pequeno = ctx.clone();
                ctx.estado.estilo |= NEGRITO;
                let titulo = inline_em("Resumo", &ctx, &mut ctx.estado.clone());
                self.push_bloco(Conteudo::Paragrafo { texto: titulo });
                if let Some(b) = self.saida.last_mut() {
                    b.atrib.alinhamento = Alinhamento::Centro;
                }
                // O texto é uma `quotation` em `\small`: recuado 2,5 em dos dois lados, sem barra nem itálico.
                let em = self.ctx.classe.tamanhos[4];
                for mut b in Analisador::novo(corpo, ctx_pequeno).executar() {
                    b.atrib.recuo_esq = 2.5 * em;
                    b.atrib.recuo_dir = 2.5 * em;
                    self.saida.push(b);
                }
            }
            "quote" | "quotation" => {
                let de = self.saida.len();
                self.citacao(corpo);
                self.espaco_em_volta(de, self.topsep());
            }
            "tabular" | "tabularx" => self.tabular(corpo),
            // A `longtable` fica no meio da linha, a não ser que o `[l]`/`[r]` diga outra coisa.
            "longtable" => {
                self.tabular(corpo);
                let lado = opcional(corpo).map(|(o, _)| o.trim()).filter(|_| corpo.trim_start().starts_with('['));
                if let Some(b) = self.saida.iter_mut().rev().find(|b| matches!(b.conteudo, Conteudo::Tabela { .. })) {
                    b.atrib.alinhamento = match lado {
                        Some("l") => Alinhamento::Esquerda,
                        Some("r") => Alinhamento::Direita,
                        _ => Alinhamento::Centro,
                    };
                }
            }
            "figure" => self.figura(corpo),
            // A tabela flutuante com imagem dentro é uma figura; com `tabular`, os blocos de dentro.
            "table" if corpo.contains("\\includegraphics") && !corpo.contains("\\begin{tabular") => self.figura(corpo),
            "table" => {
                let filhos = Analisador::novo(corpo, self.ctx.clone()).executar();
                self.saida.extend(filhos);
            }
            "center" | "flushleft" | "flushright" | "raggedright" | "justify" => {
                let mut ctx = self.ctx.clone();
                ctx.alinhamento = match base {
                    "center" => Alinhamento::Centro,
                    "flushright" => Alinhamento::Direita,
                    "justify" => Alinhamento::Justificado,
                    _ => Alinhamento::Esquerda,
                };
                // O `\vfill` do fim do ambiente vale para o bloco de depois dele.
                let (corpo, encher) = sem_vfill_no_fim(corpo);
                let filhos = Analisador::novo(corpo, ctx).executar();
                let de = self.saida.len();
                self.saida.extend(filhos);
                self.ctx.encher |= encher;
                // `center`, `flushleft` e `flushright` são uma lista de um item só (`trivlist`): o `\topsep`
                // (+ `\partopsep`) em volta. O `raggedright` e o `justify` só alinham.
                if matches!(base, "center" | "flushleft" | "flushright") {
                    let volta = self.topsep() + if self.vertical { self.ctx.classe.partopsep() } else { 0.0 };
                    self.espaco_em_volta(de, volta);
                    self.ctx.fim_de_ambiente = volta;
                }
            }
            // A capa: o que vem dentro, numa página só dela.
            "titlepage" => {
                let filhos = Analisador::novo(corpo, self.ctx.clone()).executar();
                self.saida.extend(filhos);
                self.push_bloco(Conteudo::QuebraDePagina);
            }
            "subfigure" => self.subfigura(largura, corpo),
            "tikzpicture" => self.tikz(corpo),
            "proof" => self.prova(corpo),
            "tcolorbox" => self.tcolorbox(None, corpo),
            "mdframed" => self.tcolorbox(Some(&pacotes::Caixa { args: 0, padrao: String::new(), opcoes: pacotes::MDFRAMED.to_string() }), corpo),
            t if self.ctx.teoremas.contains_key(t) => {
                let t = self.ctx.teoremas[t].clone();
                self.teorema(&t, corpo);
            }
            // Uma lista do `\newlist`: as opções do nível dela vão na frente do corpo, como se escritas ali.
            t if self.ctx.listas_novas.contains_key(t) => {
                let (numerada, niveis) = self.ctx.listas_novas[t].clone();
                let nivel = self.ctx.listas_abertas.iter().filter(|n| *n == t).count();
                let opcoes = niveis.get(nivel).or(niveis.last()).cloned().unwrap_or_default();
                let corpo = if corpo.trim_start().starts_with('[') || opcoes.is_empty() { corpo.to_string() } else { format!("[{opcoes}]{corpo}") };
                self.ctx.listas_abertas.push(t.to_string());
                self.lista(&corpo, numerada);
                self.ctx.listas_abertas.pop();
            }
            t if self.ctx.caixas.contains_key(t) => {
                let c = self.ctx.caixas[t].clone();
                self.tcolorbox(Some(&c), corpo);
            }
            // `\begin{small}`…`\end{small}`: a declaração de tamanho vale até o fim do ambiente.
            t if self.ctx.classe.tamanho_de(t).is_some() => {
                let mut ctx = self.ctx.clone();
                ctx.estado.tamanho = ctx.classe.tamanho_de(t).unwrap_or(0.0);
                let filhos = Analisador::novo(corpo, ctx).executar();
                self.saida.extend(filhos);
            }
            "minipage" => {
                let mut ctx_mini = self.ctx.clone();
                // A caixa mais estreita que a linha fica onde o ambiente de fora a põe (o `flushright`
                // a encosta à direita); dentro dela o texto volta a ser justificado.
                let sobra = comprimento(largura, &self.ctx).map_or(0.0, |w| (self.ctx.largura_util - w).max(0.0));
                let (esq, dir) = match self.ctx.alinhamento {
                    Alinhamento::Direita => (sobra, 0.0),
                    Alinhamento::Centro => (sobra / 2.0, sobra / 2.0),
                    _ => (0.0, sobra),
                };
                ctx_mini.largura_util -= sobra;
                if corpo.contains("\\centering") {
                    ctx_mini.alinhamento = Alinhamento::Centro;
                } else if sobra > 0.0 {
                    ctx_mini.alinhamento = Alinhamento::Justificado;
                }
                for mut b in Analisador::novo(corpo, ctx_mini).executar() {
                    b.atrib.recuo_esq += esq;
                    b.atrib.recuo_dir += dir;
                    self.saida.push(b);
                }
            }
            _ => self.ambiente_extra(base, corpo),
        }
    }

    fn citacao(&mut self, corpo: &str) {
        for mut c in Analisador::novo(corpo, self.ctx.clone()).executar() {
            if let Conteudo::Paragrafo { texto } = c.conteudo {
                c.conteudo = Conteudo::Citacao { texto };
                // A largura do recuo (2,5 em): a citação do LaTeX não tem itálico nem barra.
                c.latex = format!("{}", 2.5 * self.ctx.classe.tamanhos[4]);
            }
            self.saida.push(c);
        }
    }

    fn codigo(&mut self, corpo: &str, base: &str) {
        let mut corpo = corpo;
        let mut linguagem = String::new();
        let mut estilo = String::new();
        if base == "minted" {
            if let Some((l, k)) = grupo(corpo) {
                linguagem = l.to_string();
                corpo = &corpo[k..];
            }
        } else if base == "lstlisting" {
            let (locais, k) = opcional(corpo).unwrap_or(("", 0));
            corpo = &corpo[k..];
            let opcoes = opcoes_lst(&self.ctx.estilos_lst, locais);
            let valor = |chave: &str| opcoes.iter().rev().find(|(k, _)| k == chave).map(|(_, v)| v.strip_prefix('{').and_then(|v| v.strip_suffix('}')).unwrap_or(v).trim().to_string());
            linguagem = valor("language").unwrap_or_default();
            if !opcoes.is_empty() {
                estilo = self.estilo_da_listagem(&valor);
            }
            // A legenda da listagem vai em cima do código: "Listagem 1: …" (o separador das legendas).
            if let Some(legenda) = valor("caption") {
                let n = self.ctx.listagens.get() + 1;
                self.ctx.listagens.set(n);
                let separador = if self.ctx.separador_legenda.is_empty() { " –" } else { self.ctx.separador_legenda };
                let mut texto = vec![Trecho { texto: format!("Listagem {n}{separador} "), estilo: self.ctx.classe.familia | if self.ctx.legenda_negrito { NEGRITO } else { 0 }, ..Trecho::default() }];
                texto.extend(self.inline_aqui(&legenda));
                self.push_bloco(Conteudo::Paragrafo { texto });
                if let Some(b) = self.saida.last_mut() {
                    b.atrib.alinhamento = Alinhamento::Centro;
                }
            }
            if let Some(r) = valor("label") {
                self.rotular(r);
            }
        }
        if base == "lstlisting" && !self.ctx.listings_utf8 {
            if let Some(l) = corpo.lines().find(|l| !l.is_ascii()) {
                self.ctx.avisos.anotar(TipoDeAviso::Erro, &format!("Invalid UTF-8 byte sequence (listings sem literate): {}", l.trim()));
            }
        }
        let codigo = sem_tabulacao(corpo.trim_matches('\n').trim_end(), self.ctx.tabulacao);
        self.push_bloco(Conteudo::Codigo { linguagem, codigo });
        if let Some(b) = self.saida.last_mut() {
            b.latex = estilo;
        }
    }

    /// O `Bloco.latex` da listagem (formato de `layout::codigo_latex`) a partir das opções do `listings`.
    fn estilo_da_listagem(&self, valor: &dyn Fn(&str) -> Option<String>) -> String {
        let (classe, cores) = (self.ctx.classe, &self.ctx.cores);
        let cor = |chave: &str| valor(chave).and_then(|v| letra(&v, classe, cores).3);
        let basico = letra(&valor("basicstyle").unwrap_or_default(), classe, cores);
        let mut saida = format!("tam={}", basico.0.unwrap_or(classe.tamanhos[4]));
        for (nome, c) in [("texto", basico.3), ("chave", cor("keywordstyle")), ("comentario", cor("commentstyle")), ("aspas", cor("stringstyle")), ("fundo", cor("backgroundcolor"))] {
            if let Some(c) = c {
                saida.push_str(&format!(" {nome}={c:08x}"));
            }
        }
        // Os padrões do `listings`: palavra-chave em negrito e comentário em itálico.
        if valor("keywordstyle").is_some_and(|v| !v.contains("\\bfseries")) {
            saida.push_str(" sem_negrito");
        }
        if valor("commentstyle").is_none_or(|v| v.contains("\\itshape")) {
            saida.push_str(" italico");
        }
        if valor("frame").is_some_and(|f| !matches!(f.as_str(), "none" | "")) {
            saida.push_str(&format!(" moldura={:08x}", cor("rulecolor").unwrap_or(0x000000FF)));
        }
        saida
    }

    fn figura(&mut self, corpo: &str) {
        // Mais de uma imagem, subfiguras ou um desenho: os blocos de dentro, com o `\caption` de figura.
        if corpo.matches("\\includegraphics").count() != 1 || corpo.contains("\\begin{subfigure}") {
            let mut ctx = self.ctx.clone();
            ctx.em_figura = true;
            ctx.proxima_subfigura = std::rc::Rc::default();
            if corpo.contains("\\centering") {
                ctx.alinhamento = Alinhamento::Centro;
            }
            self.saida.extend(Analisador::novo(corpo, ctx).executar());
            return;
        }
        let mut largura = 0.0;
        let origem = corpo.find("\\includegraphics").and_then(|p| {
            let r = &corpo[p + "\\includegraphics".len()..];
            let r = match opcional(r) {
                Some((o, k)) => {
                    largura = largura_da_imagem(o, &self.ctx);
                    &r[k..]
                }
                None => r,
            };
            grupo(r).map(|(g, _)| g.trim().to_string())
        });
        let mut legenda = corpo.find("\\caption").and_then(|p| grupo(&corpo[p + "\\caption".len()..])).map(|(g, _)| inline_em(g, &self.ctx, &mut self.ctx.estado.clone())).unwrap_or_default();
        let Some(origem) = origem else { return };
        if self.ctx.legenda_negrito {
            legenda.iter_mut().for_each(|t| t.estilo |= NEGRITO);
        }
        // `\caption` antes da imagem (o jeito da ABNT): a legenda numerada ("Figura 1 – …") em cima.
        let em_cima = matches!((corpo.find("\\caption"), corpo.find("\\includegraphics")), (Some(c), Some(i)) if c < i);
        if em_cima && !legenda.is_empty() {
            self.push_legenda(std::mem::take(&mut legenda), 1);
        }
        let numerada = !legenda.is_empty();
        self.push_bloco(Conteudo::Imagem { origem, legenda, largura, altura: 0.0 });
        // A legenda embaixo (o padrão do LaTeX) também é numerada: "Figura 1: …", com o separador da classe.
        if let Some(b) = self.saida.last_mut().filter(|_| numerada) {
            b.latex = self.ctx.separador_legenda.to_string();
        }
        let mut r = corpo;
        while let Some(p) = r.find("\\label") {
            r = &r[p + "\\label".len()..];
            if let Some((g, _)) = grupo(r) {
                self.rotular(g.trim().to_string());
            }
        }
        // O resto da figura (a linha "Fonte: …" embaixo da imagem) continua como texto, centrado como ela.
        let mut resto = corpo.to_string();
        for cmd in ["\\includegraphics", "\\caption", "\\label"] {
            while let Some(i) = resto.find(cmd) {
                let mut j = i + cmd.len();
                while let Some((_, k)) = opcional(&resto[j..]) {
                    j += k;
                }
                j += grupo(&resto[j..]).map_or(0, |(_, k)| k);
                resto.replace_range(i..j, "");
            }
        }
        let resto = resto.replace("\\newline", "");
        let mut ctx = self.ctx.clone();
        if resto.contains("\\centering") {
            ctx.alinhamento = Alinhamento::Centro;
        }
        self.saida.extend(Analisador::novo(&resto, ctx).executar());
    }

    fn tabular(&mut self, corpo: &str) {
        let corpo = &expandir_tipos_de_coluna(corpo, &self.ctx.tipos_de_coluna);
        let sem_repetir = tabelas::sem_cabecalho_repetido(corpo);
        let mut corpo: &str = &sem_repetir;
        while let Some((_, k)) = opcional(corpo) {
            corpo = &corpo[k..];
        }
        // `{largura}{colunas}` (tabular*, tabularx) ou só `{colunas}`: um `{` depois das colunas já é a
        // primeira célula (`{\Huge 4} & …`), não outro argumento.
        let mut colunas = "";
        if let Some((g, k)) = grupo(corpo) {
            colunas = g;
            corpo = &corpo[k..];
            let largura = g.trim_start().starts_with(|c: char| c.is_ascii_digit() || c == '.' || c == '\\');
            if let Some((c, k)) = grupo(corpo).filter(|_| largura) {
                colunas = c;
                corpo = &corpo[k..];
            }
        }
        // A legenda da `longtable` é a primeira "linha": `\caption{…}\label{…}\\`.
        let mut sem_legenda = String::new();
        if let Some(c) = corpo.find("\\caption").filter(|c| corpo[..*c].trim().is_empty()) {
            let fim = corpo[c..].find("\\\\").map_or(corpo.len(), |f| c + f + 2);
            let linha = corpo[c..fim].trim_end_matches("\\\\");
            self.saida.extend(Analisador::novo(linha, self.ctx.clone()).executar());
            sem_legenda = corpo[fim..].to_string();
        }
        if !sem_legenda.is_empty() {
            corpo = &sem_legenda;
        }
        let tem_regua = ["\\hline", "\\toprule", "\\midrule", "\\bottomrule", "\\cline"].iter().any(|r| corpo.contains(r)) || colunas.contains('|');
        let cabecalho = corpo.contains("\\midrule") || corpo.matches("\\hline").count() >= 2 && corpo.find("\\hline").is_some_and(|p| p < 3);
        let (linhas, mesclagens, fios) = tabelas::linhas_de_tabela(corpo, &self.ctx);
        if linhas.len() == 1 && !tem_regua && linhas[0].len() > 1 && mesclagens.is_empty() {
            // Uma linha só, sem réguas: é arrumação de texto (nome à esquerda, data à direita), não tabela.
            let texto = linha_como_texto(&linhas[0], colunas.trim_end_matches(['@', '{', '}']).ends_with('r'));
            self.push_bloco(Conteudo::Paragrafo { texto });
        } else if !linhas.is_empty() {
            self.push_bloco(Conteudo::Tabela { cabecalho, linhas });
            if let Some(b) = self.saida.last_mut() {
                b.mesclagens = mesclagens;
                b.latex = format!("{}\n{}", colunas.trim(), fios.join(";"));
            }
        }
    }

    fn lista(&mut self, corpo: &str, ordenado: bool) {
        let nivel = self.ctx.nivel_lista;
        // `enumitem`: `[label=\alph*)]` dá a), b), c); `\roman*`, i., ii.; `\arabic*`, 1., 2.
        let mut marcador = featherdocs_core::modelo::Lista::Padrao;
        let mut modelo = None;
        let mut corpo = corpo;
        let mut inicio = None;
        if let Some((o, k)) = opcional(corpo) {
            let opcoes = opcoes_chave_valor(o);
            inicio = opcoes.iter().find(|(k, _)| k == "start").and_then(|(_, v)| v.trim().parse::<u32>().ok());
            let rotulo = opcoes.iter().find(|(k, _)| k == "label").map_or(String::new(), |(_, v)| v.clone());
            let alinhado = opcoes.iter().any(|(k, v)| k == "align" && v.trim() == "left");
            (marcador, modelo) = modelo_de_rotulo(&rotulo, opcoes.iter().any(|(k, _)| k == "resume"), alinhado);
            corpo = &corpo[k..];
        }
        // Sem rótulo dado, o do LaTeX pela profundidade do `itemize`/`enumerate` (•, –, ∗, ·; 1., (a), i., A.)
        // quando difere do que o FeatherDocs põe pelo nível da lista.
        let tipo = usize::from(ordenado);
        let prof = usize::from(self.ctx.profundidade[tipo].min(3));
        if modelo.is_none() && marcador == featherdocs_core::modelo::Lista::Padrao && !self.ctx.rotulos_do_preambulo {
            if ordenado && prof > 0 {
                (marcador, modelo) = modelo_de_rotulo(["", "(\\alph*)", "\\roman*.", "\\Alph*."][prof], false, false);
            } else if !ordenado && ["•", "–", "∗", "·"][prof] != ["•", "◦", "▪"][usize::from(nivel % 3)] {
                modelo = Some(format!("{}d|{}", if prof == 1 { "b" } else { "" }, ["•", "–", "∗", "·"][prof]));
            }
        }
        let (antes, itens) = itens_de(corpo);
        // `\begin{itemize}\small`: o que vem antes do primeiro `\item` vale para a lista toda.
        let mut e = self.ctx.estado;
        inline_em(antes, &self.ctx, &mut e);
        let mut ctx = Ctx { nivel_lista: nivel + 1, estado: e, vspace: 0.0, ..self.ctx.clone() };
        ctx.profundidade[tipo] += 1;
        // O texto do item fica onde o LaTeX o põe (`\leftmargin`); o FeatherDocs recua 18 pt por nível
        // e o resto vai no recuo do bloco. O que continua um item (o parágrafo depois de uma sublista)
        // fica alinhado ao texto dele.
        let recuo_do_texto = self.ctx.classe.recuo_da_lista(nivel);
        let a_mais = recuo_do_texto - 18.0 * f32::from(nivel.min(5) + 1);
        let recuo_de_fora = if nivel == 0 { 0.0 } else { self.ctx.classe.recuo_da_lista(nivel - 1) };
        for (k, item) in itens.into_iter().enumerate() {
            let mut filhos = Analisador::novo(item, ctx.clone()).executar().into_iter();
            let (texto, outro) = match filhos.next() {
                Some(Bloco { conteudo: Conteudo::Paragrafo { texto }, .. }) => (texto, None),
                Some(outro) => (Vec::new(), Some(outro)),
                None => (Vec::new(), None),
            };
            self.push_bloco(Conteudo::Item { ordenado, nivel: nivel.min(5), texto });
            if let Some(b) = self.saida.last_mut() {
                b.atrib.lista = marcador;
                b.latex = modelo.clone().unwrap_or_default();
                // Cada `enumerate` conta do 1 (o `resume` segue pelo modelo do rótulo).
                b.atrib.reiniciar = k == 0;
                if k == 0 && ordenado {
                    b.atrib.mais.inicio = inicio;
                }
                b.atrib.recuo_esq += a_mais;
                // O rótulo de texto (`item_com_rotulo`) fica onde começa o texto de fora e empurra só a
                // primeira linha.
                if modelo.is_some() {
                    b.atrib.primeira = -(recuo_do_texto - recuo_de_fora);
                }
                // O vão vem antes também: depois do parágrafo que continua o item anterior ele não some.
                if k > 0 && b.atrib.antes.is_none() {
                    b.atrib.antes = b.atrib.depois;
                }
            }
            let item = self.saida.len() - 1;
            for (j, mut b) in outro.into_iter().chain(filhos).enumerate() {
                if !matches!(b.conteudo, Conteudo::Item { .. }) {
                    b.atrib.recuo_esq = b.atrib.recuo_esq.max(recuo_do_texto);
                } else if j == 0 {
                    // A sublista logo depois do texto do item: o vão é o dela, não o entre itens.
                    self.saida[item].atrib.depois = Some(0.0);
                }
                self.saida.push(b);
            }
        }
    }
}

/// Separa o corpo de uma lista nos `\item` do nível dele (ignora os de listas aninhadas); devolve
/// também o que vem antes do primeiro item.
fn itens_de(corpo: &str) -> (&str, Vec<&str>) {
    let mut itens = Vec::new();
    let mut prof = 0;
    let mut inicio: Option<usize> = None;
    let mut j = 0;
    while j < corpo.len() {
        let r = &corpo[j..];
        if r.starts_with("\\begin{") {
            prof += 1;
        } else if r.starts_with("\\end{") {
            prof -= 1;
        } else if prof == 0 && r.starts_with("\\item") && !r[5..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            if let Some(a) = inicio {
                itens.push(&corpo[a..j]);
            }
            let mut ini = j + 5;
            if let Some((_, k)) = opcional(&corpo[ini..]) {
                ini += k;
            }
            inicio = Some(ini);
            j = ini;
            continue;
        }
        j += r.chars().next().map_or(1, char::len_utf8);
    }
    if let Some(a) = inicio {
        itens.push(&corpo[a..]);
    }
    let antes = corpo.find("\\item").map_or(corpo, |i| &corpo[..i]);
    (antes, itens)
}

/// As células de uma linha como texto corrido: a última vai para a direita (`\t`) se a coluna é `r`.
fn linha_como_texto(celulas: &[Celula], ultima_a_direita: bool) -> Vec<Trecho> {
    let mut saida: Vec<Trecho> = Vec::new();
    for (k, c) in celulas.iter().enumerate() {
        if k > 0 {
            let sep = if ultima_a_direita && k + 1 == celulas.len() { "\t" } else { " " };
            saida.push(Trecho { texto: sep.into(), ..c.first().cloned().unwrap_or_default() });
            if let Some(t) = saida.last_mut() {
                t.link = None;
            }
        }
        saida.extend(c.iter().cloned());
    }
    saida
}

fn dividir_celulas(linha: &str) -> Vec<String> {
    let mut celulas = vec![String::new()];
    let mut anterior = ' ';
    for c in linha.chars() {
        if c == '&' && anterior != '\\' {
            celulas.push(String::new());
        } else if let Some(u) = celulas.last_mut() {
            u.push(c);
        }
        anterior = c;
    }
    celulas
}

#[cfg(test)]
mod testes {
    use super::*;
    use featherdocs_core::edicao::nome_do_tipo;

    fn tipos(d: &Documento) -> Vec<String> {
        d.blocos.iter().map(|b| nome_do_tipo(&b.conteudo)).collect()
    }

    #[test]
    fn documento_completo() {
        let d = ler(
            "\\documentclass{article}\n\\title{Meu \\textbf{artigo}}\\author{Ana}\n\\begin{document}\n\\maketitle\n\\section{Introdu\\c{c}\\~ao}\nTexto com \\emph{\\'enfase} e $x^2$.\n\n\\subsection*{Sub}\nOutro.\n\\end{document}",
        );
        assert_eq!(d.meta.titulo, "Meu artigo");
        // O `\maketitle`: título, autor e data (a de hoje, sem `\date`) centrados.
        assert_eq!(tipos(&d), vec!["paragrafo", "paragrafo", "paragrafo", "titulo1", "paragrafo", "titulo2", "paragrafo"]);
        assert_eq!(texto_puro(d.blocos[0].conteudo.textos()[0]), "Meu artigo");
        assert_eq!(d.blocos[1].atrib.alinhamento, Alinhamento::Centro);
        assert!(matches!(d.blocos[2].conteudo.textos()[0][0].marca.as_deref(), Some(Marca::Campo { campo }) if campo == "data"));
        let Conteudo::Titulo { texto, .. } = &d.blocos[3].conteudo else { panic!() };
        assert_eq!(texto_puro(texto), "Introdução");
    }

    #[test]
    fn listas_aninhadas() {
        let d = ler("\\begin{itemize}\\item a \\item b\n\\begin{enumerate}\\item c\\end{enumerate}\n\\item d\\end{itemize}");
        let v: Vec<(bool, u8)> = d.blocos.iter().map(|b| if let Conteudo::Item { ordenado, nivel, .. } = b.conteudo { (ordenado, nivel) } else { (false, 99) }).collect();
        assert_eq!(v, vec![(false, 0), (false, 0), (true, 1), (false, 0)]);
    }

    #[test]
    fn paragrafo_depois_da_sublista_continua_o_item() {
        let d = ler("\\begin{enumerate}\\item Um:\n\\begin{itemize}\\item a\\end{itemize}\n\nSegue o um.\n\\item Dois.\\end{enumerate}\n\nFora.\n\n\\begin{enumerate}\\item Outra.\\end{enumerate}");
        let marcas: Vec<String> = featherdocs_core::paginacao::marcadores(&d.blocos, &[]).into_iter().flatten().collect();
        assert_eq!([&marcas[0], &marcas[2], &marcas[3]], ["1.", "2.", "1."]);
        let segue = d.blocos.iter().find(|b| texto_puro(b.conteudo.textos()[0]).starts_with("Segue")).unwrap();
        // Alinhado ao texto do item: o `\leftmargin` de fora (2,5em no corpo 10).
        assert_eq!(segue.atrib.recuo_esq, 25.0);
    }

    #[test]
    fn formulas_codigo_e_tabela() {
        let d = ler("\\[ E = mc^2 \\]\n\\begin{equation}\\label{a} x \\end{equation}\n\\begin{verbatim}\nint a;\n\\end{verbatim}\n\\begin{tabular}{|l|l|}\\hline A & B \\\\ \\hline 1 & 2 \\\\ \\hline\\end{tabular}");
        assert_eq!(tipos(&d), vec!["equacao", "equacao", "codigo", "tabela"]);
        assert!(matches!(&d.blocos[1].conteudo, Conteudo::Equacao { latex } if latex == "\\begin{equation}\\label{a} x\\end{equation}"));
        let Conteudo::Tabela { cabecalho, linhas } = &d.blocos[3].conteudo else { panic!() };
        assert!(*cabecalho);
        assert_eq!(linhas.len(), 2);
    }

    #[test]
    fn tabular_com_grupo_na_primeira_celula_nao_perde_a_celula() {
        let d = ler("\\begin{tabular}{cc}{\\Huge 4} & {\\Huge 41} \\\\ a & b \\\\\\end{tabular}");
        let Conteudo::Tabela { linhas, .. } = &d.blocos[0].conteudo else { panic!("{:?}", d.blocos[0].conteudo) };
        assert_eq!(texto_puro(&linhas[0][0]), "4");
        assert_eq!(d.blocos[0].latex.lines().next(), Some("cc"));
    }

    #[test]
    fn figura_com_legenda() {
        let d = ler("\\begin{figure}\\includegraphics[width=3cm]{img/a.png}\\caption{Uma figura}\\end{figure}");
        let Conteudo::Imagem { origem, legenda, .. } = &d.blocos[0].conteudo else { panic!() };
        assert_eq!(origem, "img/a.png");
        assert_eq!(texto_puro(legenda), "Uma figura");
    }

    #[test]
    fn comentarios_e_pontuacao() {
        let d = ler("Antes % some\ndepois -- e --- ``aspas'' 50\\% \\& mais.\n");
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!() };
        assert_eq!(texto_puro(texto), "Antes depois – e — “aspas” 50% & mais.");
    }

    #[test]
    fn link_e_codigo_em_linha() {
        let d = ler("Veja \\href{http://x.org}{o site} e \\texttt{ls -l}.");
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!() };
        assert_eq!(texto[1].link.as_deref(), Some("http://x.org"));
        assert!(texto.iter().any(|t| t.estilo & CODIGO != 0 && t.texto == "ls -l"));
    }

    #[test]
    fn capitulos_empurram_as_secoes() {
        let d = ler("\\chapter{A}\\section{B}");
        assert!(matches!(d.blocos[0].conteudo, Conteudo::Titulo { nivel: 1, .. }));
        assert!(matches!(d.blocos[1].conteudo, Conteudo::Titulo { nivel: 2, .. }));
    }

    #[test]
    fn lixo_nao_trava() {
        for f in ["\\begin{itemize}", "\\section{", "$$ sem fim", "\\href{a", "{{{", "\\", "%"] {
            let _ = ler(f);
        }
    }

    #[test]
    fn exportar_e_reler() {
        let md = featherdocs_core::inline::analisar("Texto **forte** e *ênfase*.");
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Titulo { nivel: 1, texto: featherdocs_core::inline::analisar("Intro") });
        d.acrescentar(Conteudo::Paragrafo { texto: md });
        let volta = ler(&crate::exportar::latex(&d));
        assert_eq!(crate::exportar::texto_simples(&d), crate::exportar::texto_simples(&volta));
    }

    #[test]
    fn notas_e_indice_viram_objetos_e_voltam() {
        let d = ler("\\documentclass{article}\\begin{document}\nGato\\index{gato!felino}\\footnote{Um \\textbf{bicho}.} e cão\\endnote{Do fim.}.\n\\end{document}");
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!("{:?}", d.blocos) };
        let marcas: Vec<&Marca> = texto.iter().filter_map(|t| t.marca.as_deref()).collect();
        assert!(matches!(marcas[0], Marca::Indice { termo, sub } if termo == "gato" && sub == "felino"));
        assert!(matches!(marcas[1], Marca::Nota { fim: false, texto } if featherdocs_core::inline::serializar(texto) == "Um **bicho**."), "{marcas:?}");
        assert!(matches!(marcas[2], Marca::Nota { fim: true, .. }));
        assert_eq!(featherdocs_core::inline::texto_puro(texto), "Gato e cão.");
        let tex = crate::exportar::latex(&d);
        let volta = ler(&tex);
        let Conteudo::Paragrafo { texto: t2 } = &volta.blocos[0].conteudo else { panic!() };
        assert_eq!(t2.iter().filter(|t| t.marca.is_some()).count(), 3, "{tex}");
    }

    #[test]
    fn citacoes_fontes_e_listas_geradas_vao_e_voltam_pelo_latex() {
        let tex = r"\begin{filecontents*}{\jobname.bib}
@book{silva2023, author={Silva, Ana}, title={Livro}, year={2023}}
\end{filecontents*}
\documentclass{article}
\usepackage[style=apa]{biblatex}
\begin{document}
Como diz \cite[p.~4]{silva2023} e \parencite{a, b}.

\nocite{*}
\printbibliography[heading=bibintoc]

\printindex
\end{document}";
        let d = ler(tex);
        assert_eq!(d.bibliografia.estilo, EstiloDeCitacao::Apa);
        assert_eq!(d.bibliografia.fontes.len(), 1);
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!("{:?}", d.blocos) };
        let m: Vec<&Marca> = texto.iter().filter_map(|t| t.marca.as_deref()).collect();
        assert!(matches!(m[0], Marca::Citacao { chaves, pagina } if chaves == &["silva2023"] && pagina == "4"), "{m:?}");
        assert!(matches!(m[1], Marca::Citacao { chaves, .. } if chaves == &["a", "b"]));
        let tipos: Vec<String> = d.blocos.iter().map(|b| featherdocs_core::edicao::nome_do_tipo(&b.conteudo)).collect();
        assert_eq!(tipos, ["paragrafo", "bibliografia", "indice_remissivo"]);
        assert!(matches!(d.blocos[1].conteudo, Conteudo::Bibliografia { todas: true }));
        let c = chaves_no_codigo(r"\nocite{d} \citeauthor{a, b} e \cite[p. 4]{c} \citeyear{e}");
        assert_eq!(c.len(), 5, "{c:?}");
        let volta = ler(&crate::exportar::latex(&d));
        assert_eq!(volta.bibliografia, d.bibliografia);
        assert_eq!(volta.blocos.len(), 3);
    }

    #[test]
    fn icone_do_fontawesome_vira_imagem_na_cor_do_texto() {
        let d = ler("\\begin{document}Texto {\\color{red}\\faCircleInfo} e \\faLightbulb[regular] fim.\\end{document}");
        let t = &d.blocos[0].conteudo.textos()[0];
        let imagens: Vec<&String> = t.iter().filter_map(|x| match x.marca.as_deref() { Some(Marca::Imagem { origem, largura, .. }) if *largura > 5.0 => Some(origem), _ => None }).collect();
        assert_eq!(imagens.len(), 2);
        let svg = String::from_utf8(crate::base64::de_data_url(imagens[0]).unwrap().1).unwrap();
        assert!(svg.contains("#e03030"), "{svg}");
        assert_eq!(texto_puro(t), "Texto  e  fim.");
    }

    #[test]
    fn estilo_do_listings_vai_para_o_bloco_de_codigo() {
        let d = ler("\\usepackage{listings,xcolor}\\lstdefinestyle{base}{basicstyle=\\ttfamily\\footnotesize,keywordstyle=\\color{blue}\\bfseries,frame=single,rulecolor=\\color{gray}}\\lstdefinestyle{bash}{language=bash,style=base}\\lstset{backgroundcolor=\\color{white}}\\begin{document}\\begin{lstlisting}[style=bash]\necho oi\n\\end{lstlisting}\\end{document}");
        let b = d.blocos.iter().find(|b| matches!(b.conteudo, Conteudo::Codigo { .. })).unwrap();
        assert!(matches!(&b.conteudo, Conteudo::Codigo { linguagem, .. } if linguagem == "bash"));
        assert!(b.latex.starts_with("tam=8 ") && b.latex.contains(" fundo=ffffffff") && b.latex.contains(" moldura=") && b.latex.contains(" italico"), "{}", b.latex);
        assert!(!b.latex.contains("sem_negrito"), "{}", b.latex);
    }

    #[test]
    fn rotulo_padrao_vai_pela_profundidade_do_itemize_e_do_enumerate() {
        let d = ler("\\begin{document}\\begin{enumerate}\\item a\\begin{itemize}\\item b\\begin{itemize}\\item c\\end{itemize}\\end{itemize}\\begin{enumerate}\\item d\\end{enumerate}\\end{enumerate}\\end{document}");
        let rotulos: Vec<&str> = d.blocos.iter().map(|b| b.latex.as_str()).collect();
        assert_eq!(rotulos, ["", "d|•", "bd|–", "d|(#a)"]);
    }

    #[test]
    fn hidelinks_deixa_o_link_preto_e_o_url_monoespacado() {
        let d = ler("\\PassOptionsToPackage{hidelinks}{hyperref}\\usepackage{hyperref}\\begin{document}Escreva para \\url{a@b.br}.\\end{document}");
        let t = d.blocos[0].conteudo.textos()[0].iter().find(|t| t.link.is_some()).unwrap();
        assert_eq!((t.texto.as_str(), t.fmt.cor, t.estilo & CODIGO), ("a@b.br", 0x000000FF, CODIGO));
    }

    #[test]
    fn titulos_no_tamanho_da_classe_e_do_titleformat() {
        let d = ler("\\documentclass[11pt]{article}\\usepackage{titlesec}\\definecolor{az}{HTML}{1F4E79}\\titleformat{\\subsection}{\\large\\bfseries\\color{az}}{\\thesubsection}{1em}{}\\begin{document}\\section{A}\\subsection{B}\\end{document}");
        let e = |i: usize| featherdocs_core::modelo::Estilo::do_latex(i as u8 + 1, &d.blocos[i].latex).unwrap();
        assert_eq!((e(0).tamanho, e(0).bits, e(0).cor), (14.4, NEGRITO | SERIFA, 0));
        assert_eq!((e(1).tamanho, e(1).bits, e(1).cor), (12.0, NEGRITO | SERIFA, 0x1F4E79FF));
        assert!(e(0).antes.unwrap() > e(1).antes.unwrap());
    }

    #[test]
    fn secao_sem_estrela_liga_a_numeracao_e_a_com_estrela_fica_fora() {
        let d = ler("\\begin{document}\\section*{Prefácio}\\section{Um}\\subsection{Dois}\\end{document}");
        assert_eq!(d.estilos.numeracao, Numeracao::Decimal);
        let fora: Vec<bool> = d.blocos.iter().map(|b| b.atrib.sem_numero).collect();
        assert_eq!(fora, [true, false, false]);
        assert_eq!(ler("\\section*{A}").estilos.numeracao, Numeracao::Nenhuma);
        assert_eq!(ler("\\chapter{A}\\section{B}").estilos.numeracao, Numeracao::Capitulo);
    }

    #[test]
    fn numeracao_vai_e_volta_pelo_latex() {
        let d = ler("\\begin{document}\\section*{Prefácio}\\section{Um}\\subsection{Dois}\\end{document}");
        let tex = crate::exportar::latex(&d);
        assert!(tex.contains("\\section*{Pref") && tex.contains("\\section{Um}") && tex.contains("\\subsection{Dois}"), "{tex}");
        let volta = ler(&tex);
        assert_eq!(volta.estilos.numeracao, d.estilos.numeracao);
        assert_eq!(volta.blocos.iter().map(|b| b.atrib.sem_numero).collect::<Vec<_>>(), [true, false, false]);
        let mut sem = d.clone();
        sem.estilos.numeracao = Numeracao::Nenhuma;
        assert!(crate::exportar::latex(&sem).contains("\\section*{Um}"));
    }

    #[test]
    fn cores_macros_e_colorbox() {
        let tex = r#"
\definecolor{darkblue}{HTML}{1A3A5C}
\definecolor{midblue}{HTML}{2E6DA4}
\newcommand{\destaque}[1]{\textbf{\color{midblue}#1}}
\begin{document}
\colorbox{darkblue}{
  \begin{minipage}{10cm}
    \centering
    \color{white}Meu Nome
  \end{minipage}
}
\destaque{Importante}
\end{document}
"#;
        let d = ler(tex);
        assert!(!d.blocos.is_empty());
        // O primeiro bloco tem fundo darkblue (0x1A3A5CFF)
        assert_eq!(d.blocos[0].atrib.fundo, 0x1A3A5CFF);
        assert_eq!(d.blocos[0].atrib.alinhamento, Alinhamento::Centro);
        // O texto "Meu Nome" é branco (0xFFFFFFFF)
        if let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo {
            assert!(texto.iter().any(|t| t.fmt.cor == 0xFFFFFFFF && t.texto.contains("Meu Nome")));
        } else {
            panic!("esperado parágrafo");
        }
        // O macro \destaque virou negrito com midblue (0x2E6DA4FF)
        if let Conteudo::Paragrafo { texto } = &d.blocos[1].conteudo {
            assert!(texto.iter().any(|t| t.fmt.cor == 0x2E6DA4FF && (t.estilo & NEGRITO != 0)));
        } else {
            panic!("esperado parágrafo");
        }
    }

    /// Um resumo do `curriculo_marcelo.tex` da raiz (o que o leitor tem que acertar).
    const CURRICULO: &str = r#"
\documentclass[10pt, a4paper]{article}
\usepackage[utf8]{inputenc}
\usepackage[T1]{fontenc}
\usepackage[portuguese]{babel}
\usepackage[a4paper, top=13mm, bottom=13mm, left=17mm, right=17mm]{geometry}
\setlength{\parindent}{0pt}
\setlength{\parskip}{0pt}
\pagestyle{empty}
\usepackage{helvet}
\renewcommand{\familydefault}{\sfdefault}
\usepackage[dvipsnames, table]{xcolor}
\definecolor{darkblue}{HTML}{1A3A5C}
\definecolor{midblue}{HTML}{2E6DA4}
\definecolor{lightblue}{HTML}{D6E8F7}
\definecolor{lightgray}{HTML}{F4F6F9}
\definecolor{graytext}{HTML}{555555}
\definecolor{darktext}{HTML}{1C1C1C}
\usepackage{tabularx}
\usepackage{colortbl}
\usepackage{array}
\usepackage{enumitem}
\newcommand{\secline}{
  \vspace{1pt}
  {\color{midblue}\hrule height 0.8pt}
  \vspace{4pt}
}
\newcommand{\seccao}[1]{
  \vspace{6pt}
  {\large\bfseries\color{darkblue}\MakeUppercase{#1}}
  \par\secline
}
\newcommand{\expheader}[3]{
  \vspace{4pt}
  \noindent
  \colorbox{lightgray}{
    \begin{tabularx}{\dimexpr\textwidth-2\fboxsep\relax}{@{}X r@{}}
      \textbf{\color{midblue}#1}\;{\color{graytext}|\ #2} &
      {\small\itshape\color{graytext}#3}
    \end{tabularx}
  }
  \par\vspace{2pt}
}
\newcommand{\edu}[3]{
  \noindent\textbf{#1} --- {\color{midblue}#2} \hfill {\small\itshape\color{graytext}#3}\par
  \vspace{2pt}
}
\newcommand{\skill}[2]{
  \item \textbf{\color{midblue}#1:} {\color{darktext}#2}
}
\begin{document}
\noindent
\colorbox{darkblue}{
  \begin{minipage}{\dimexpr\textwidth-2\fboxsep\relax}
    \vspace{8pt}
    \centering
    \textbf{\LARGE\color{white}Marcelo A. Gonçalves}\\[3pt]
    {\color{lightblue}Engenheiro Eletricista · Cientista de Dados \& Automação}
    \vspace{8pt}
  \end{minipage}
}\par

\seccao{Perfil Profissional}
Engenheiro Eletricista com \textbf{CREA ativo}.

\seccao{Experiência Profissional}
\expheader{SANDECH}{Engenheiro}{Jan 2026 -- Atualmente}
\begin{itemize}
  \item \textbf{Automação com Python e VBA:} redução do lead time.
\end{itemize}

\seccao{Habilidades Técnicas}
\begin{itemize}
  \skill{Linguagens}{Python (Pandas, SQLAlchemy, Flask), SQL, VBA, JavaScript}
\end{itemize}
\end{document}
"#;

    #[test]
    fn curriculo_marcelo_completo() {
        let tex = CURRICULO;
        let d = ler(tex);
        let par = |b: &Bloco| match &b.conteudo {
            Conteudo::Paragrafo { texto } | Conteudo::Item { texto, .. } => texto.clone(),
            _ => Vec::new(),
        };
        // Papel e margens do geometry; sem número de página (\pagestyle{empty}).
        assert!((d.pagina.margem_esq - 17.0 * 72.0 / 25.4).abs() < 0.01);
        assert!((d.pagina.margem_sup - 13.0 * 72.0 / 25.4).abs() < 0.01);
        assert!(d.extras.rodape.vazia());
        // A faixa do cabeçalho: o \\ parte em dois parágrafos, os dois com o fundo e centralizados.
        let (nome, sub) = (&d.blocos[0], &d.blocos[1]);
        for b in [nome, sub] {
            assert_eq!(b.atrib.fundo, 0x1A3A5CFF);
            assert_eq!(b.atrib.alinhamento, Alinhamento::Centro);
        }
        let t = par(nome);
        assert_eq!(texto_puro(&t), "Marcelo A. Gonçalves");
        assert!(t.iter().all(|t| t.fmt.cor == 0xFFFFFFFF && t.fmt.tamanho == 17.28 && t.estilo & NEGRITO != 0), "{t:?}");
        assert!(par(sub).iter().any(|t| t.fmt.cor == 0xD6E8F7FF && t.texto.contains("Engenheiro Eletricista") && t.fmt.tamanho == 10.0));
        assert_eq!(sub.atrib.antes, Some(0.0));
        // Seção: \large (12 pt na classe 10pt), negrito, darkblue e em maiúsculas sem estragar comandos.
        let secao = d.blocos.iter().find(|b| texto_puro(&par(b)) == "PERFIL PROFISSIONAL").expect("seção");
        assert!(par(secao).iter().all(|t| t.fmt.tamanho == 12.0 && t.estilo & NEGRITO != 0 && t.fmt.cor == 0x1A3A5CFF));
        // A régua da seção na cor midblue.
        assert!(d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::Regua) && b.atrib.cor_borda == 0x2E6DA4FF));
        // A tarja da experiência: um parágrafo com fundo, o período encostado à direita (tabulação).
        let tarja = d.blocos.iter().find(|b| b.atrib.fundo == 0xF4F6F9FF).expect("tarja");
        let t = texto_puro(&par(tarja));
        assert!(t.starts_with("SANDECH | Engenheiro\tJan 2026"), "{t:?}");
        // O texto corrido sai justificado, como no LaTeX; o cabeçalho continua centralizado.
        let perfil = d.blocos.iter().find(|b| texto_puro(&par(b)).starts_with("Engenheiro Eletricista com")).expect("perfil");
        assert_eq!(perfil.atrib.alinhamento, Alinhamento::Justificado);
                // O item da habilidade: texto em darktext.
        assert!(d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::Item { .. }) && par(b).iter().any(|t| t.fmt.cor == 0x1C1C1CFF)));
    }

    /// O que um TCC na abnTeX2 usa (do artigo de exemplo do usuário): margens da ABNT, número da
    /// página no canto pelo `fancyhdr`, `\fontsize`, `\citeauthor`/`\citeyear`, `\ref`, a linha
    /// "Fonte:" da figura, `table` com `[h]`, seção em maiúsculas e `\\` dentro do parágrafo.
    #[test]
    fn legenda_em_negrito_e_hspace() {
        let tex = "\\documentclass{article}\n\\usepackage{caption}\n\\captionsetup{font=bf}\n\\begin{document}\n\\hspace{1cm}Recuado\n\n\\begin{table}\n\\caption{Dados}\n\\begin{tabular}{c}\na \\\\\n\\end{tabular}\n\\end{table}\n\\end{document}\n";
        let l = ler_com_avisos(tex);
        assert!(l.avisos.is_empty(), "{:?}", l.avisos);
        let d = &l.documento;
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!("{:?}", d.blocos) };
        assert!(texto_puro(texto).starts_with('\u{a0}') && texto_puro(texto).ends_with("Recuado"), "{texto:?}");
        let Conteudo::Paragrafo { texto } = &d.blocos[1].conteudo else { panic!() };
        assert!(d.blocos[1].atrib.legenda == 2 && texto[0].estilo & NEGRITO != 0);
    }

    #[test]
    fn erros_que_o_pdflatex_tambem_daria() {
        let tex = "\\documentclass{article}\n\\usepackage{listings}\n\\begin{document}\n\\begin{equation}\nx=1\\label{a}\\label{b}\n\\end{equation}\n\\begin{lstlisting}\n# mês\n\\end{lstlisting}\n\\end{document}";
        let erros: Vec<_> = ler_com_avisos(tex).avisos.into_iter().filter(|a| a.tipo == TipoDeAviso::Erro).map(|a| (a.nome, a.linha)).collect();
        assert_eq!(erros.len(), 2, "{erros:?}");
        assert!(erros.iter().any(|(n, l)| n.contains("Multiple") && *l == 5), "{erros:?}");
        assert!(erros.iter().any(|(n, l)| n.contains("UTF-8") && *l == 8), "{erros:?}");
        // Com o `literate` no preâmbulo, o acento no código passa.
        let ok = tex.replace("\\begin{document}", "\\lstset{literate={ê}{{\\^e}}1}\n\\begin{document}");
        assert!(ler_com_avisos(&ok).avisos.iter().all(|a| !a.nome.contains("UTF-8")));
        // Os avisos do log: babel 'brazil' e biblatex sem csquotes.
        let tex = "\\documentclass{article}\n\\usepackage[brazil]{babel}\n\\usepackage{biblatex}\n\\begin{document}\nx\n\\end{document}";
        let avisos: Vec<_> = ler_com_avisos(tex).avisos.into_iter().filter(|a| a.tipo == TipoDeAviso::AvisoDoLatex).map(|a| a.linha).collect();
        assert_eq!(avisos, vec![2, 3]);
    }

    #[test]
    fn pacotes_de_relatorio() {
        let d = ler(r"\documentclass{article}
\usepackage{amsthm,tcolorbox,siunitx,cleveref}
\definecolor{fundo}{HTML}{F2F4F7}
\newtheorem{definicao}{Definição}[section]
\newtcolorbox{nota}[1][]{colback=fundo,colframe=black,title=#1}
\begin{document}
\section{Um}\label{sec:um}
\begin{definicao}\emph{HSP} vale \SI{1000}{\watt\per\square\meter}.\end{definicao}
\begin{nota}[Título]Texto.\end{nota}
\begin{enumerate}[label=\alph*)]\item a\item b\end{enumerate}
\rowcolors{2}{fundo}{white}
\begin{tabular}{|l|c|c|}\hline
\multirow{2}{*}{X} & \multicolumn{2}{c|}{Y} \\ \cline{2-3}
 & 1 & 2 \\[2pt] \hline
\end{tabular}
\begin{longtable}{ll}\caption{Cronograma.}\label{tab:c}\\
A & B \\ \endfirsthead A & B \\ \endhead 1 & 2 \\
\end{longtable}
\begin{figure}\begin{subfigure}[b]{0.5\textwidth}\includegraphics[width=\linewidth]{a.png}\caption{Primeira.}\end{subfigure}\hfill
\begin{subfigure}[b]{0.5\textwidth}\includegraphics[width=\linewidth]{b.png}\caption{Segunda.}\end{subfigure}
\caption{As duas.}\label{fig:duas}\end{figure}
Veja a \cref{sec:um}, a \Cref{fig:duas}, a \cref{tab:c} e \enquote{isto}.
\end{document}");
        let textos: Vec<String> = d.blocos.iter().map(|b| b.conteudo.textos().into_iter().map(|t| texto_puro(t)).collect::<Vec<_>>().join("|")).collect();
        assert!(textos[1].starts_with("Definição 1.1. HSP vale 1000\u{a0}W/m²."), "{textos:?}");
        // A caixa: o título na faixa da moldura e o texto no fundo, com a moldura em volta.
        assert_eq!((textos[2].as_str(), d.blocos[2].atrib.fundo, d.blocos[3].atrib.fundo), ("Título", 0x000000FF, 0xF2F4F7FF));
        assert_eq!((d.blocos[2].atrib.borda, d.blocos[3].atrib.borda), (1 | 2 | 4 | 16, 1 | 2 | 8 | 16));
        assert!(d.blocos[3].atrib.recuo_esq > 12.0 && d.blocos[3].atrib.recuo_dir > 12.0);
        assert_eq!(d.blocos[4].atrib.lista, featherdocs_core::modelo::Lista::Alfabetica);
        let Conteudo::Tabela { linhas, .. } = &d.blocos[6].conteudo else { panic!("{textos:?}") };
        assert_eq!((linhas.len(), linhas[0].len(), linhas[1].len()), (2, 3, 3));
        assert_eq!(d.blocos[6].mesclagens, vec![featherdocs_core::modelo::MesclagemTabela { linha: 0, coluna: 1, colunas: 2 }]);
        assert_eq!((d.blocos[6].atrib.fundo, d.blocos[6].atrib.marcado), (0xF2F4F7FF, false));
        // A `longtable`: a legenda numerada em cima e o cabeçalho uma vez só.
        assert_eq!((textos[7].as_str(), d.blocos[7].atrib.legenda), ("Cronograma.", 2));
        let Conteudo::Tabela { linhas, .. } = &d.blocos[8].conteudo else { panic!() };
        assert_eq!(linhas.len(), 2);
        assert_eq!((textos[9].as_str(), textos[10].as_str(), textos[11].as_str()), ("(a) Primeira.", "(b) Segunda.", "As duas."));
        assert_eq!(d.blocos[11].atrib.legenda, 1);
        assert_eq!(textos[12], "Veja a seção 1, a Figura 1, a tabela 1 e “isto”.");
        // E volta pela exportação: a mesclagem, as linhas alternadas, a lista com letras, a moldura e os links.
        let tex = crate::exportar::latex(&d);
        for trecho in ["\\multicolumn{2}", "\\rowcolors{2}{linhaF2F4F7}{white}", "\\begin{enumerate}[label=\\alph*)]", "\\fcolorbox[HTML]{000000}{F2F4F7}", "\\hyperref[bloco:"] {
            assert!(tex.contains(trecho), "falta {trecho} em:\n{tex}");
        }
        let r = ler(&tex);
        let tabela = r.blocos.iter().find(|b| matches!(b.conteudo, Conteudo::Tabela { .. })).unwrap();
        assert_eq!((tabela.mesclagens.len(), tabela.atrib.fundo), (1, 0xF2F4F7FF));
        assert!(r.blocos.iter().any(|b| b.atrib.lista == featherdocs_core::modelo::Lista::Alfabetica));
        let ultimo = r.blocos.iter().rev().find(|b| matches!(b.conteudo, Conteudo::Paragrafo { .. })).unwrap();
        let alvo = ultimo.conteudo.textos()[0].iter().find_map(|t| t.link.clone()).unwrap();
        assert!(alvo.starts_with("#bloco:"), "{alvo}");
    }

    #[test]
    fn tcc_na_abntex2() {
        let tex = "\\documentclass[12pt,a4paper,section=TITLE,brazil]{abntex2}\n\\usepackage{fancyhdr}\n\\usepackage[alf]{abntex2cite}\n\\fancyhf{}\n\\fancyhead[R]{\\fontsize{10}{12}\\selectfont\\thepage}\n\\begin{filecontents*}{x.bib}\n@misc{edp2024, author={EDP}, title={Mapa}, year={2024}}\n@article{ab, author={Oliveira, Ana and Ferreira, Bia}, title={T}, year={2022}}\n\\end{filecontents*}\n\\begin{document}\n\\pagenumbering{arabic}\n\\pagestyle{fancy}\n{\\fontsize{12}{14}\\selectfont \\textbf{TÍTULO}}\n\n\\section{Introdução}\nVer a Figura \\ref{fig:a} e \\citeauthor{ab} (\\citeyear{ab}).\\\\\nOutra linha.\n\\begin{figure}[h]\n\\centering\n\\caption{Mapa}\n\\includegraphics[width=1\\linewidth]{mapa.png}\n\\small{Fonte: \\citeauthor{edp2024}, \\citeyear{edp2024}.}\n\\label{fig:a}\n\\end{figure}\n\\begin{table}[!htpb]\n\\renewcommand{\\arraystretch}{1.5}\n\\begin{tabular}{cc}\n\\hline\na & b \\\\ \\hline\n\\end{tabular}\n\\end{table}\n\\end{document}\n";
        let l = ler_com_avisos(tex);
        let d = &l.documento;
        assert!(l.avisos.is_empty(), "{:?}", l.avisos);
        // O `\ref` leva à figura do `\label`.
        let figura = d.blocos.iter().find(|b| matches!(b.conteudo, Conteudo::Imagem { .. })).unwrap().id;
        let links: Vec<_> = d.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().filter_map(|t| t.link.clone()).collect();
        assert!(links.contains(&format!("#bloco:{figura}")), "{links:?}");
        assert!((d.pagina.margem_esq - 85.04).abs() < 0.1 && (d.pagina.margem_dir - 56.69).abs() < 0.1);
        assert_eq!(d.extras.cabecalho.direita, "{pagina}");
        assert!(d.extras.rodape.vazia());
        let textos: Vec<String> = d.blocos.iter().map(|b| match &b.conteudo {
            Conteudo::Paragrafo { texto } | Conteudo::Titulo { texto, .. } => texto_puro(texto),
            Conteudo::Imagem { origem, .. } => format!("[{origem}]"),
            Conteudo::Tabela { linhas, .. } => format!("tabela {}", linhas.len()),
            _ => String::new(),
        }).collect();
        assert_eq!(textos, ["TÍTULO", "INTRODUÇÃO", "Ver a Figura 1 e OLIVEIRA; FERREIRA (2022).\u{2028}Outra linha.", "Mapa", "[mapa.png]", "Fonte: EDP, 2024.", "tabela 1"]);
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!() };
        assert_eq!(texto[0].fmt.tamanho, 12.0);
        // O `\caption` antes da imagem é a legenda numerada em cima ("Figura 1 – Mapa").
        assert_eq!((d.blocos[3].atrib.legenda, d.blocos[3].atrib.alinhamento), (1, Alinhamento::Centro));
        let Conteudo::Imagem { largura, ref legenda, .. } = d.blocos[4].conteudo else { panic!() };
        assert!((largura - d.pagina.largura_util()).abs() < 0.1 && legenda.is_empty());
        // `\arraystretch` 1.5 estica as linhas da tabela.
        assert!((d.blocos[6].atrib.entrelinha - 1.725).abs() < 0.001);
    }

    #[test]
    fn classe_local_com_ambientes_setters_e_listas_com_rotulo() {
        let cls = "\\NeedsTeXFormat{LaTeX2e}\n\\ProvidesClass{minha}\n\\LoadClass{article}\n\\RequirePackage{util}\n\\ifPDFTeX\\def\\motor{pdf}\\else\\def\\motor{lua}\\fi\n\\global\\let\\@nota\\@empty\n\\newcommand{\\nota}[1]{\\def\\@nota{#1}}\n\\newenvironment{norma}[1]{\\textbf{#1}\\par\\ifx\\@nota\\@empty\\else Nota: \\@nota\\fi}{Fim.}\n\\newlist{artigo}{enumerate}{2}\n\\setlist[artigo,1]{resume,label=\\textbf{Art. \\arabic*º}}\n";
        let sty = "\\newenvironmentx{aviso}[2][1=Atenção,2=x]{\\textbf{#1} }{}";
        let tex = "\\documentclass{minha}\n\\begin{document}\n\\nota{lida}\n\\begin{norma}{Título}\nCorpo.\n\\end{norma}\n\\begin{aviso}Cuidado.\\end{aviso}\n\\begin{artigo}\\item Um.\\end{artigo}\nMeio.\n\\begin{artigo}\\item Dois.\\end{artigo}\n\\end{document}\n";
        let arquivos = |n: &str| match n {
            "minha.cls" => Some(cls.as_bytes().to_vec()),
            "util.sty" => Some(sty.as_bytes().to_vec()),
            _ => None,
        };
        let l = ler_projeto(tex, &arquivos);
        assert!(l.avisos.is_empty(), "{:?}", l.avisos.iter().map(|a| a.descricao()).collect::<Vec<_>>());
        let textos: Vec<String> = l.documento.blocos.iter().map(|b| b.conteudo.textos().into_iter().map(|t| texto_puro(t)).collect::<Vec<_>>().join("|")).collect();
        assert_eq!(textos[..2], ["Título", "Nota: lida Corpo. Fim. Atenção Cuidado."]);
        let marcas = featherdocs_core::paginacao::marcadores(&l.documento.blocos, &[]).into_iter().flatten().collect::<Vec<_>>();
        assert_eq!(marcas, ["Art. 1º", "Art. 2º"]);
    }

    #[test]
    fn scale_da_letra_sem_serifa_diminui_a_letra_e_nao_a_entrelinha() {
        let c = Classe::do_preambulo("\\documentclass[11pt]{article}\\usepackage[scale=0.88]{tgheros}\\renewcommand*\\familydefault{\\sfdefault}");
        assert_eq!(c.escala, 0.88);
        assert!((c.entrelinha() * 0.88 - Classe::do_preambulo("\\documentclass[11pt]{article}").entrelinha()).abs() < 1e-4);
        // Com a letra romana de padrão o `scale` não mexe no texto.
        assert_eq!(Classe::do_preambulo("\\usepackage[scale=0.88]{tgheros}").escala, 1.0);
        assert_eq!(Classe::do_preambulo("\\setmainfont[Scale = .9,]{TeX Gyre Heros}").escala, 0.9);
    }

    #[test]
    fn lipsum_tem_o_tamanho_do_pacote() {
        // Os parágrafos 1 e 2 do pacote têm 867 e 550 letras; o nosso fica a menos de uma palavra disso.
        for (n, t) in [(1, 867), (2, 550), (151, 867)] {
            let p = lipsum(n);
            assert!(p.len() <= t + 1 && p.len() + 16 > t && p.ends_with('.'), "{n}: {} {p}", p.len());
        }
    }

    #[test]
    fn etoolbox_ifthen_lipsum_e_atribuicoes_do_tex() {
        let cls = "\\LoadClass{article}\n\\RequirePackage{etoolbox}\n\\newcommand{\\lista}{}\n\\newcommandx*\\membro[2][2=sim,usedefault]{\\ifthenelse{\\equal{#2}{sim}}{\\ifdefempty{\\lista}{\\eappto\\lista{#1}}{\\eappto\\lista{, #1}}\\csdef{todos}{\\lista}}{}}\n\\newcommand{\\fim}{\\ifthenelse{\\equal{\\lista}{}}{Ninguém.}{Presentes: \\csuse{todos}.}}\n\\parskip=0.3\\baselineskip \\advance\\parskip by 0pt plus 2pt\n\\hbadness=10000\n";
        let tex = "\\documentclass{ata}\n\\title{Ata}\n\\begin{document}\n\\membro{Ana}\\membro{Bia}[nao]\\membro{Caio}\n\\fim\n\n\\textsc{Ata} e \\censor{X}.\n\n\\lipsum[2]\n\\end{document}\n";
        let arquivos = |n: &str| (n == "ata.cls").then(|| cls.as_bytes().to_vec());
        let l = ler_projeto(tex, &arquivos);
        assert!(l.avisos.is_empty(), "{:?}", l.avisos.iter().map(|a| a.descricao()).collect::<Vec<_>>());
        let textos: Vec<String> = l.documento.blocos.iter().map(|b| b.conteudo.textos().into_iter().map(|t| texto_puro(t)).collect::<Vec<_>>().join("|")).collect();
        assert_eq!(textos[0], "Presentes: Ana, Caio.");
        assert_eq!(textos[1], "ATA e X.");
        assert!(textos[2].starts_with("Nulla facilisi."), "{}", textos[2]);
        let versalete: Vec<f32> = l.documento.blocos[1].conteudo.textos()[0].iter().take(2).map(|t| t.fmt.tamanho).collect();
        assert!(versalete[0] == 0.0 || versalete[0] > versalete[1], "{versalete:?}");
        // Como na ata do IFSC: os membros entram no preâmbulo e a lista sai no fim do texto.
        let tex = "\\documentclass{ata}\n\\membro{Ana}\n\\membro{Caio}\n\\begin{document}\n\\fim\n\\end{document}\n";
        let l = ler_projeto(tex, &arquivos);
        let texto: String = l.documento.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().map(|t| t.texto.as_str()).collect();
        assert_eq!(texto.trim(), "Presentes: Ana, Caio.");
    }

    #[test]
    fn parindent_recua_o_paragrafo_novo_e_nao_o_do_titulo() {
        let d = ler("\\documentclass[11pt]{article}\n\\begin{document}\n\\section{A}\nPrimeiro.\n\nSegundo.\n\\[x\\]\nem que x.\n\n\\noindent Sem.\n\\end{document}\n");
        let recuos: Vec<f32> = d.blocos.iter().filter(|b| matches!(b.conteudo, Conteudo::Paragrafo { .. })).map(|b| b.atrib.primeira).collect();
        assert_eq!(recuos, [0.0, 17.0, 0.0, 0.0]);
        let d = ler("\\documentclass{article}\\usepackage{parskip}\\begin{document}\nUm.\n\nDois.\n\\end{document}");
        assert!(d.blocos.iter().all(|b| b.atrib.primeira == 0.0));
    }

    #[test]
    fn appendix_numera_em_letras_e_o_ref_acompanha() {
        let d = ler(r"\documentclass{article}
\begin{document}
\section{Um}
\section{Dois}
Veja o \ref{ap}.
\appendix
\section{Memória}\label{ap}
\subsection{Parte}
\end{document}");
        let n: Vec<String> = featherdocs_core::paginacao::numeros_dos_titulos(&d.blocos, d.estilos.numeracao, "").into_iter().flatten().collect();
        assert_eq!(n, ["1", "2", "A", "A.1"]);
        let texto: String = d.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().map(|t| t.texto.as_str()).collect();
        assert!(texto.contains("Veja o A."), "{texto}");
    }

    #[test]
    fn hyperref_pinta_os_links_e_fancyhdr_poe_o_fio() {
        let d = ler(r"\documentclass{article}
\usepackage{hyperref}
\hypersetup{colorlinks=true,linkcolor=NavyBlue,urlcolor=Maroon}
\usepackage{fancyhdr}
\pagestyle{fancy}
\fancyhead[L]{Topo}
\begin{document}
\section{A}\label{a}
Ver \ref{a} e \href{https://x.y}{site}.
\end{document}");
        let cores: Vec<u32> = d.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().filter(|t| t.link.is_some()).map(|t| t.fmt.cor).collect();
        assert_eq!(cores, [0x0F75FFFF, 0xAD0000FF]);
        let x = &d.extras;
        assert_eq!((x.cabecalho.esquerda.as_str(), x.cabecalho.fio, x.rodape.fio, x.cabecalho.cor), ("Topo", 0.4, 0.0, 0x000000FF));
    }

    #[test]
    fn fancypagestyle_com_logo_tamanho_e_primeira_pagina() {
        // Um PNG de 200 × 20 pixels (só o cabeçalho: o leitor mede por ele).
        let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13];
        png.extend(b"IHDR");
        png.extend([0, 0, 0, 200, 0, 0, 0, 20, 8, 2, 0, 0, 0, 0, 0, 0, 0]);
        let fonte = r"\documentclass[11pt]{article}
\usepackage{fancyhdr}
\newcommand{\setor}[1]{\def\nomesetor{#1}}
\newcommand{\mostrasetor}{\nomesetor}
\pgfdeclareimage[width=1.27\linewidth]{logo}{logo}
\fancypagestyle{primeira}{\fancyhf{}\renewcommand{\headrulewidth}{0pt}\fancyhead[C]{\pgfuseimage{logo}}\fancyfoot[R]{\begin{footnotesize}página \thepage~de \pageref{LastPage}\end{footnotesize}}}
\fancypagestyle{seguintes}{\fancyhf{}\fancyhead[R]{\begin{tiny}\mostrasetor\end{tiny}}}
\fancypagestyle{seguintes}{\fancyfoot[L]{IFSC}}
\pagestyle{seguintes}
\AtBeginDocument{\thispagestyle{primeira}}
\setor{Colegiado}
\begin{document}
Texto.
\end{document}";
        let x = ler_projeto(fonte, &|n| (n == "logo.png").then(|| png.clone())).documento.extras;
        assert_eq!((x.cabecalho.direita.as_str(), x.cabecalho.tamanho, x.rodape.esquerda.as_str()), ("Colegiado", 6.0, "IFSC"));
        let p = &x.cabecalho_primeira;
        assert!(x.primeira_diferente && p.imagem.starts_with("data:image/png"));
        // 1,27 × 360 pt: o `\linewidth` da classe de 11 pt no preâmbulo.
        assert!((p.imagem_largura - 457.2).abs() < 0.01 && (p.imagem_altura - 45.72).abs() < 0.01, "{p:?}");
        assert_eq!((x.rodape_primeira.direita.as_str(), x.rodape_primeira.tamanho), ("página {pagina}\u{a0}de {total}", 9.0));
    }

    #[test]
    fn vfill_e_clearpage_da_capa() {
        let d = ler(r"\documentclass{article}
\begin{document}
\begin{titlepage}
\begin{center}{\Huge Título\par}\vfill\end{center}
\begin{flushright}{Autor\par}\vfill\end{flushright}
\begin{center}{Data\par}\end{center}
\end{titlepage}
\clearpage
\tableofcontents
\clearpage
Texto.
\end{document}");
        let resumo: Vec<(bool, &str)> = d
            .blocos
            .iter()
            .map(|b| match &b.conteudo {
                Conteudo::QuebraDePagina => (b.atrib.encher, "quebra"),
                Conteudo::Sumario { .. } => (b.atrib.encher, "sumário"),
                c => (b.atrib.encher, c.textos().into_iter().flatten().map(|t| t.texto.as_str()).next().unwrap_or("")),
            })
            .collect();
        // Uma quebra só depois da capa (o `\clearpage` numa página nova não faz nada).
        assert_eq!(resumo, [(false, "Título"), (true, "Autor"), (true, "Data"), (false, "quebra"), (false, "sumário"), (false, "quebra"), (false, "Texto.")]);
        let x = &d.extras;
        assert!(x.primeira_diferente && x.rodape_primeira.vazia(), "a capa sai sem rodapé");
    }

    #[test]
    fn printbibliography_com_titulo_e_palavra_chave() {
        let bib = b"@book{a, author={Silva, Ana}, title={A}, year={2020}, keywords={basica}}\n@book{b, author={Lima, Rui}, title={B}, year={2021}, keywords={complementar}}";
        let fonte = r"\documentclass{article}
\usepackage{biblatex}
\addbibresource{r.bib}
\begin{document}
\section{Ementa}
Texto.
\nocite{*}
\printbibliography[heading=bibintoc,keyword={basica},title={Bibliografia básica}]
\printbibliography[keyword=complementar,title={Bibliografia complementar}]
\section{Depois}
\end{document}";
        let d = ler_projeto(fonte, &|n| (n == "r.bib").then(|| bib.to_vec())).documento;
        let resumo: Vec<(String, bool, &str)> = d
            .blocos
            .iter()
            .map(|b| match &b.conteudo {
                Conteudo::Titulo { texto, .. } => (texto_puro(texto), b.atrib.sem_numero, ""),
                Conteudo::Bibliografia { todas: true } => ("lista".to_string(), false, b.latex.as_str()),
                _ => (String::new(), false, ""),
            })
            .filter(|r| !r.0.is_empty())
            .collect();
        let esperado = [("Ementa", false, ""), ("Bibliografia básica", true, ""), ("lista", false, "keyword=basica"), ("Bibliografia complementar", true, ""), ("lista", false, "keyword=complementar"), ("Depois", false, "")];
        assert_eq!(resumo, esperado.map(|(t, e, l)| (t.to_string(), e, l)));
        assert_eq!(d.bibliografia.fontes.iter().map(|f| f.palavras.as_str()).collect::<Vec<_>>(), ["basica", "complementar"]);
    }

    #[test]
    fn espacos_de_listas_e_center_como_no_latex() {
        let d = ler(r"\documentclass[11pt]{article}
\begin{document}
\begin{center}
{\Large\textbf{Título}}\\[1ex]
Data
\end{center}
\vspace{2ex}\par\noindent\textbf{Informes}\par
\begin{enumerate}
\item Um.
\begin{itemize}\item A.\item B.\end{itemize}
Continua.
\item Dois.
\end{enumerate}
\vspace{2ex}\par Depois.
\end{document}");
        let classe = Classe::do_preambulo("\\documentclass[11pt]{article}");
        let ex2 = dimensao("2ex", classe).unwrap();
        let b = |texto: &str| d.blocos.iter().find(|b| b.conteudo.textos().into_iter().flatten().any(|t| t.texto.contains(texto))).unwrap_or_else(|| panic!("{texto}: {:?}", d.blocos)).atrib;
        let perto = |a: Option<f32>, b: f32| a.is_some_and(|a| (a - b).abs() < 0.01);
        // O `center` (topsep 9 + partopsep 3) soma ao `\vspace` de depois; o `\\[1ex]` vale para a linha seguinte.
        assert!(perto(b("Informes").antes, 12.0 + ex2), "{:?}", b("Informes"));
        assert!(b("Data").antes.is_some_and(|a| a > 3.0));
        // A lista depois de `\par`: topsep + partopsep. Entre itens de fora 9, de dentro 4; o item depois do
        // parágrafo que continua o anterior também tem o vão.
        assert!(perto(b("Um.").antes, 12.0) && perto(b("A.").depois, 4.0) && perto(b("Dois.").antes, 9.0));
        assert!(perto(b("Depois.").antes, 12.0 + ex2), "{:?}", b("Depois."));
        // Recuo do texto: 2,5em e mais 2,2em (o FeatherDocs já põe 18 pt por nível).
        let em = 10.95;
        assert!((b("Um.").recuo_esq - (2.5 * em - 18.0)).abs() < 0.01 && (b("A.").recuo_esq - (4.7 * em - 36.0)).abs() < 0.01);
        assert!((b("Continua.").recuo_esq - 2.5 * em).abs() < 0.01);
        assert_eq!(b("Depois.").alinhamento, Alinhamento::Justificado);
    }

    #[test]
    fn date_troca_a_data_da_classe_e_chave_escapada_nao_abre_grupo() {
        let cls = br"\def\@date{\today}\newcommand{\mostradata}{\@date}";
        let fonte = r"\documentclass{local}
\date{03 de dezembro de 2019}
\begin{document}
\mostradata
\end{document}";
        let d = ler_projeto(fonte, &|n| (n == "local.cls").then(|| cls.to_vec())).documento;
        assert_eq!(texto_puro(d.blocos[0].conteudo.textos()[0]), "03 de dezembro de 2019");
        // O `literate={\}}{…}` do listings e o `\RaggedRight` de uma coluna: dentro de grupo.
        assert!(!fora_de_grupo(r"\lstset{literate={\}}{x}1}\newcolumntype{L}{>{\RaggedRight}p{1cm}}", "\\RaggedRight"));
        assert!(fora_de_grupo(r"\lstset{literate={\}}{x}1}\RaggedRight", "\\RaggedRight"));
    }

    #[test]
    fn estilos_do_memoir_viram_cabecalho_e_rodape() {
        let x = ler(r"\documentclass{memoir}
\makepagestyle{plain}
\makeoddfoot{plain}{}{\thepage}{}
\makeoddhead{headings}{}{Topo}{}
\makeoddfoot{headings}{}{\thepage}{}
\begin{document}
Texto.
\end{document}")
        .extras;
        assert_eq!((x.cabecalho.centro.as_str(), x.cabecalho.fio, x.rodape.centro.as_str()), ("Topo", 0.0, "{pagina}"));
    }
}

