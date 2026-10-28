//! Pacotes e definições do preâmbulo; `\input`/`\include`; ambientes além dos básicos.

use std::collections::HashMap;

use featherdocs_core::modelo::{Bloco, Conteudo, Trecho, ITALICO, NEGRITO};

use super::avisos::{self, Coletor, TipoDeAviso};
use super::{comando, grupo, opcional, Analisador};

/// A classe e os pacotes que o leitor não conhece entram no aviso.
pub(super) fn avisar_preambulo(preambulo: &str, coletor: &Coletor) {
    if let Some(c) = avisos::classe_do_preambulo(preambulo) {
        if !avisos::classe_conhecida(&c) {
            coletor.anotar(TipoDeAviso::Classe, &c);
        }
    }
    let pacotes = avisos::pacotes_do_preambulo(preambulo);
    for p in &pacotes {
        if !avisos::pacote_conhecido(p) {
            coletor.anotar(TipoDeAviso::Pacote, p);
        }
    }
    // Os avisos que o próprio LaTeX escreve no log para estes preâmbulos.
    if let Some(p) = preambulo.find("{babel}") {
        let linha = &preambulo[preambulo[..p].rfind('\\').unwrap_or(0)..p + 7];
        if linha.contains("brazil]") || linha.contains("brazil,") {
            coletor.anotar(TipoDeAviso::AvisoDoLatex, &format!("Package babel Warning: Name 'brazil' is deprecated, use 'brazilian': {linha}"));
        }
    }
    let tem = |n: &str| pacotes.iter().any(|p| p == n);
    if tem("biblatex") && (tem("babel") || tem("polyglossia")) && !tem("csquotes") {
        coletor.anotar(TipoDeAviso::AvisoDoLatex, "Package biblatex Warning: 'babel/polyglossia' detected but 'csquotes' missing: {biblatex}");
    }
}

impl Analisador<'_> {
    /// Ambiente que os básicos não tratam: o conteúdo sai como texto e o nome vai para o aviso.
    pub(super) fn ambiente_extra(&mut self, base: &str, corpo: &str) {
        if base != "document" {
            self.ctx.avisos.ambiente(base);
        }
        let filhos = Analisador::novo(corpo, self.ctx.clone()).executar();
        self.saida.extend(filhos);
    }
}

/// Um ambiente do `\newtheorem`: o nome que sai ("Teorema"), o estilo do `\theoremstyle` em vigor
/// (0 `plain`, 1 `definition`, 2 `remark`), o contador (o próprio ou o de outro ambiente), se ele
/// recomeça a cada seção e se é numerado (`\newtheorem*` não é).
#[derive(Clone, Debug)]
pub(super) struct Teorema {
    pub nome: String,
    pub estilo: u8,
    pub contador: String,
    pub por_secao: bool,
    pub numerado: bool,
}

/// Os `\newtheorem` do preâmbulo, na ordem (o `\theoremstyle` vale para os seguintes).
pub(super) fn teoremas_do_preambulo(preambulo: &str) -> HashMap<String, Teorema> {
    let mut saida = HashMap::new();
    let mut estilo = 0;
    let mut i = 0;
    while let Some(p) = preambulo[i..].find('\\') {
        i += p;
        let (nome, n) = comando(&preambulo[i..]);
        i += n;
        let r = &preambulo[i..];
        if nome == "theoremstyle" {
            if let Some((g, _)) = grupo(r) {
                estilo = match g.trim() {
                    "definition" => 1,
                    "remark" => 2,
                    _ => 0,
                };
            }
        } else if nome == "newtheorem" {
            let numerado = !preambulo[..i].ends_with('*');
            let Some((env, a)) = grupo(r) else { continue };
            let mut k = a;
            let mut contador = env.trim().to_string();
            if let Some((o, b)) = opcional(&r[k..]) {
                contador = o.trim().to_string();
                k += b;
            }
            let Some((titulo, b)) = grupo(&r[k..]) else { continue };
            k += b;
            let por_secao = opcional(&r[k..]).is_some_and(|(o, _)| matches!(o.trim(), "section" | "chapter"));
            let nome = super::texto_puro(&super::inline(titulo, &HashMap::new())).trim().to_string();
            saida.insert(env.trim().to_string(), Teorema { nome, estilo, contador, por_secao, numerado });
        }
    }
    saida
}

/// Uma caixa do `\newtcolorbox{nome}[1][padrão]{opções}`: quantos argumentos, o padrão do
/// opcional e as opções (com `#1` no lugar do argumento).
#[derive(Clone, Debug)]
pub(super) struct Caixa {
    pub args: usize,
    pub padrao: String,
    pub opcoes: String,
}

pub(super) fn caixas_do_preambulo(preambulo: &str) -> HashMap<String, Caixa> {
    let mut saida = HashMap::new();
    let mut i = 0;
    while let Some(p) = preambulo[i..].find("\\newtcolorbox") {
        i += p + "\\newtcolorbox".len();
        let r = &preambulo[i..];
        let Some((nome, mut k)) = grupo(r) else { continue };
        let mut args = 0;
        let mut padrao = String::new();
        if let Some((o, b)) = opcional(&r[k..]) {
            args = o.trim().parse().unwrap_or(0);
            k += b;
            if let Some((o, b)) = opcional(&r[k..]) {
                padrao = o.to_string();
                k += b;
            }
        }
        let Some((opcoes, _)) = grupo(&r[k..]) else { continue };
        saida.insert(nome.trim().to_string(), Caixa { args, padrao, opcoes: opcoes.to_string() });
    }
    // `\tcbset{nome/.style={…}}`: um conjunto de opções com nome.
    let mut i = 0;
    while let Some(p) = preambulo[i..].find("\\tcbset") {
        i += p + "\\tcbset".len();
        let Some((g, _)) = grupo(&preambulo[i..]) else { continue };
        for (k, v) in super::opcoes_chave_valor(g) {
            if let Some(nome) = k.strip_suffix("/.style") {
                let v = v.trim();
                let v = v.strip_prefix('{').and_then(|v| v.strip_suffix('}')).unwrap_or(v);
                saida.insert(format!("@estilo:{}", nome.trim()), Caixa { args: 0, padrao: String::new(), opcoes: v.to_string() });
            }
        }
    }
    // `\newmdenv[opções]{nome}` do `mdframed`: a mesma caixa, com os padrões dele.
    let mut i = 0;
    while let Some(p) = preambulo[i..].find("\\newmdenv") {
        i += p + "\\newmdenv".len();
        let r = &preambulo[i..];
        let (opcoes, k) = opcional(r).unwrap_or(("", 0));
        let Some((nome, _)) = grupo(&r[k..]) else { continue };
        saida.insert(nome.trim().to_string(), Caixa { args: 0, padrao: String::new(), opcoes: format!("{MDFRAMED},{opcoes}") });
    }
    saida
}

/// Os grupos `{a}{b}{c}` seguidos de um valor.
fn grupos_de(v: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut r = v;
    while let Some((g, k)) = grupo(r) {
        saida.push(g.trim().to_string());
        r = &r[k..];
    }
    saida
}

/// Os padrões do `mdframed` nas chaves do `tcolorbox`: fundo branco, fio preto, cantos retos.
pub(super) const MDFRAMED: &str = "colback=white,colframe=black,coltitle=black,colbacktitle=white,sharp corners";

/// As chaves do `mdframed` com o nome que o `tcolorbox` dá.
fn chave_do_tcolorbox(k: &str) -> &str {
    match k {
        "frametitle" => "title",
        "frametitlebackgroundcolor" => "colbacktitle",
        "backgroundcolor" => "colback",
        "linecolor" => "colframe",
        "fontcolor" => "colupper",
        _ => k,
    }
}

impl Analisador<'_> {
    /// `\begin{teorema}[nota] … \end{teorema}`: "Teorema 1 (nota)." em negrito e o texto em
    /// itálico (no estilo `plain`; `definition` em pé, `remark` com a cabeça em itálico).
    pub(super) fn teorema(&mut self, t: &Teorema, corpo: &str) {
        let (nota, corpo) = match opcional(corpo) {
            Some((o, k)) => (Some(o), &corpo[k..]),
            None => (None, corpo),
        };
        let numero = if t.numerado { self.ctx.numeros_teoremas.borrow_mut().pop_front() } else { None };
        let familia = self.ctx.classe.familia;
        let negrito = familia | if t.estilo == 2 { ITALICO } else { NEGRITO };
        let mut cabeca = vec![Trecho { texto: format!("{}{}", t.nome, numero.map_or(String::new(), |n| format!(" {n}"))), estilo: negrito, ..Trecho::default() }];
        if let Some(n) = nota {
            cabeca.push(Trecho { texto: " (".into(), estilo: familia, ..Trecho::default() });
            cabeca.extend(self.inline_aqui(n));
            cabeca.push(Trecho { texto: ")".into(), estilo: familia, ..Trecho::default() });
        }
        cabeca.push(Trecho { texto: ". ".into(), estilo: negrito, ..Trecho::default() });
        let mut ctx = self.ctx.clone();
        if t.estilo == 0 {
            ctx.estado.estilo |= ITALICO;
        }
        let de = self.saida.len();
        self.com_cabeca(cabeca, corpo, ctx, None);
        self.espaco_em_volta(de, self.topsep());
    }

    /// `proof`: "Demonstração." em itálico e o quadradinho no fim (o `\qed`).
    pub(super) fn prova(&mut self, corpo: &str) {
        let familia = self.ctx.classe.familia;
        let (titulo, corpo) = match opcional(corpo) {
            Some((o, k)) => (self.inline_aqui(o), &corpo[k..]),
            None => (vec![Trecho { texto: "Demonstração".into(), estilo: familia, ..Trecho::default() }], corpo),
        };
        let mut cabeca: Vec<Trecho> = titulo.into_iter().map(|t| Trecho { estilo: t.estilo | ITALICO, ..t }).collect();
        cabeca.push(Trecho { texto: ". ".into(), estilo: familia | ITALICO, ..Trecho::default() });
        // O `\qed` na ponta direita: o □ vem da fonte das fórmulas (a do texto não o tem).
        let fim = vec![Trecho { texto: "\t".into(), estilo: familia, ..Trecho::default() }, Trecho { texto: "\\square".into(), estilo: featherdocs_core::modelo::MATEMATICA, ..Trecho::default() }];
        let de = self.saida.len();
        self.com_cabeca(cabeca, corpo, self.ctx.clone(), Some(fim));
        self.espaco_em_volta(de, self.topsep());
    }

    /// Os blocos de `corpo` com `cabeca` no começo do primeiro parágrafo e `fim` no último.
    fn com_cabeca(&mut self, cabeca: Vec<Trecho>, corpo: &str, ctx: super::Ctx, fim: Option<Vec<Trecho>>) {
        let mut filhos = Analisador::novo(corpo.trim(), ctx).executar();
        match filhos.first_mut().map(|b| &mut b.conteudo) {
            Some(Conteudo::Paragrafo { texto }) => {
                texto.splice(0..0, cabeca);
            }
            _ => {
                self.push_bloco(Conteudo::Paragrafo { texto: cabeca });
            }
        }
        if let (Some(f), Some(Conteudo::Paragrafo { texto })) = (fim, filhos.last_mut().map(|b| &mut b.conteudo)) {
            texto.extend(f);
        }
        self.saida.extend(filhos);
    }

    /// `\begin{tcolorbox}[opções]` ou uma caixa do `\newtcolorbox`: o título numa faixa da cor da
    /// moldura (`colframe`, letra `coltitle`) e o texto no fundo `colback`, tudo com a moldura em volta.
    pub(super) fn tcolorbox(&mut self, caixa: Option<&Caixa>, corpo: &str) {
        let mut corpo = corpo;
        let mut opcoes = caixa.map_or(String::new(), |c| c.opcoes.clone());
        if let Some(c) = caixa.filter(|c| c.args > 0) {
            let arg = match opcional(corpo) {
                Some((o, k)) => {
                    corpo = &corpo[k..];
                    o.to_string()
                }
                None => c.padrao.clone(),
            };
            opcoes = opcoes.replace("#1", &arg);
            // Os outros argumentos (`[3][]` = um opcional e dois obrigatórios) vêm em `{…}`.
            for n in 2..=c.args.min(9) {
                let Some((g, k)) = grupo(corpo) else { break };
                opcoes = opcoes.replace(&format!("#{n}"), g);
                corpo = &corpo[k..];
            }
        } else if let Some((o, k)) = opcional(corpo) {
            opcoes = if opcoes.is_empty() { o.to_string() } else { format!("{opcoes},{o}") };
            corpo = &corpo[k..];
        }
        // Um estilo do `\tcbset{nome/.style={…}}` usado pelo nome vira as opções dele.
        let o: Vec<(String, String)> = super::opcoes_chave_valor(&opcoes)
            .into_iter()
            .flat_map(|(k, v)| match self.ctx.caixas.get(&format!("@estilo:{k}")).filter(|_| v.is_empty()) {
                Some(e) => super::opcoes_chave_valor(&e.opcoes),
                None => vec![(k, v)],
            })
            .map(|(k, v)| (chave_do_tcolorbox(&k).to_string(), v))
            .collect();
        let valor = |chave: &str| o.iter().rev().find(|(k, _)| k == chave).map(|(_, v)| {
            let v = v.trim();
            v.strip_prefix('{').and_then(|v| v.strip_suffix('}')).unwrap_or(v).trim().to_string()
        });
        let cor = |chave: &str, padrao: u32| valor(chave).and_then(|v| super::resolver_cor(&self.ctx.cores, &v)).unwrap_or(padrao);
        // Os padrões do `tcolorbox`: fundo `black!5`, moldura `black!75`, título em branco.
        let (mut fundo, mut moldura, cor_titulo) = (cor("colback", 0xF2F2F2FF), cor("colframe", 0x404040FF), cor("coltitle", 0xFFFFFFFF));
        // `borderline west={3pt}{0pt}{cor}` (ou `boxrule=0` com `leftrule`): só a barra da esquerda.
        // Sem o `valor`, que tiraria a chave de fora do primeiro grupo e a de dentro do último.
        let barra = o.iter().rev().find(|(k, _)| k == "borderline west").map(|(_, v)| grupos_de(v.trim()).get(2).cloned().unwrap_or_default());
        let so_barra = barra.is_some() || (valor("boxrule").is_some_and(|v| super::dimensao(&v, self.ctx.classe) == Some(0.0)) && valor("leftrule").is_some());
        if let Some(c) = barra.and_then(|c| super::resolver_cor(&self.ctx.cores, &c)) {
            moldura = c;
        }
        if valor("opacityback").is_some_and(|v| v.trim() == "0") {
            fundo = 0;
        }
        let transparente = valor("opacitybacktitle").is_some_and(|v| v.trim() == "0");
        let fundo_titulo = if transparente { 0 } else { cor("colbacktitle", moldura) };
        let mut blocos = Vec::new();
        if let Some(t) = valor("title").filter(|t| !t.is_empty()) {
            let mut ctx = self.ctx.clone();
            ctx.estado.cor = cor_titulo;
            if valor("fonttitle").is_some_and(|f| f.contains("bf")) {
                ctx.estado.estilo |= NEGRITO;
            }
            let texto = super::inline_em(&t, &ctx, &mut ctx.estado.clone());
            let mut b = Bloco { id: 0, atrib: self.atrib_para(&Conteudo::Paragrafo { texto: Vec::new() }), conteudo: Conteudo::Paragrafo { texto }, mesclagens: Vec::new(), latex: String::new(), tabela: Default::default() };
            b.atrib.fundo = fundo_titulo;
            blocos.push(b);
        }
        blocos.extend(Analisador::novo(corpo.trim(), self.ctx.clone()).executar());
        // Como no `tcolorbox`: a moldura dentro das margens, o texto a 4 mm dela, cantos redondos.
        let recuo = 4.5 * 72.0 / 25.4;
        let redonda = !so_barra && !o.iter().any(|(k, _)| k == "sharp corners") && valor("arc").is_none_or(|v| super::dimensao(&v, self.ctx.classe) != Some(0.0));
        for b in &mut blocos {
            b.atrib.recuo_esq += recuo;
            b.atrib.recuo_dir += recuo;
            if redonda {
                b.atrib.borda |= featherdocs_core::layout::CAIXA_ARREDONDADA;
            }
        }
        let de = self.saida.len();
        self.caixa_de_blocos(blocos, fundo, moldura);
        if so_barra {
            for b in &mut self.saida[de..] {
                b.atrib.borda &= !(2 | 4 | 8);
            }
        }
        // O `before skip`/`after skip` do `tcolorbox` (o `\medskip`).
        self.espaco_em_volta(de, 6.0);
    }

    /// Os blocos de uma caixa (`\colorbox`, `\fcolorbox`, `tcolorbox`) como uma faixa só: sem espaço
    /// entre eles, com `fundo` onde não há outro e a moldura (se tiver cor) em volta de tudo.
    pub(super) fn caixa_de_blocos(&mut self, blocos: Vec<Bloco>, fundo: u32, moldura: u32) {
        let n = blocos.len();
        let mut depois_anterior = 0.0f32;
        for (k, mut f) in blocos.into_iter().enumerate() {
            if f.atrib.fundo == 0 {
                f.atrib.fundo = fundo;
            }
            // O vão entre blocos dentro da caixa vira espaço por dentro dela (a moldura não abre).
            let vao = depois_anterior.max(f.atrib.antes.unwrap_or(0.0));
            depois_anterior = f.atrib.depois.unwrap_or(0.0);
            if k > 0 && vao > 0.0 && f.latex.is_empty() && f.atrib.legenda == 0 && matches!(f.conteudo, Conteudo::Paragrafo { .. }) {
                f.latex = format!("topo={vao}");
            }
            if k > 0 {
                f.atrib.antes = Some(0.0);
            }
            if k + 1 < n {
                f.atrib.depois = Some(0.0);
            }
            if moldura != 0 {
                f.atrib.borda |= 1 | 2 | if k == 0 { 4 } else { 0 } | if k + 1 == n { 8 } else { 0 };
                f.atrib.cor_borda = moldura;
            }
            self.saida.push(f);
        }
    }
}

impl Analisador<'_> {
    /// `\begin{subfigure}[b]{0.48\textwidth}`: a imagem com a largura dela e a legenda "(a) …".
    /// shortcut: as subfiguras ficam uma embaixo da outra (o FeatherDocs não põe imagens lado a lado).
    pub(super) fn subfigura(&mut self, largura: &str, corpo: &str) {
        let letra = self.ctx.proxima_subfigura.get();
        self.ctx.proxima_subfigura.set(letra.saturating_add(1));
        let mut ctx = self.ctx.clone();
        if let Some(l) = super::comprimento(largura, &self.ctx).filter(|l| *l > 0.0) {
            ctx.largura_util = l;
        }
        ctx.subfigura = Some(char::from(b'a' + letra.min(25)));
        ctx.em_figura = true;
        if corpo.contains("\\centering") {
            ctx.alinhamento = super::Alinhamento::Centro;
        }
        self.saida.extend(Analisador::novo(corpo, ctx).executar());
    }

    /// `tikzpicture`: o desenho vira uma imagem SVG; o que o leitor não desenha vai para o aviso.
    pub(super) fn tikz(&mut self, corpo: &str) {
        let cores = self.ctx.cores.clone();
        let cor = move |n: &str| super::resolver_cor(&cores, n);
        let Some((svg, largura, altura, faltou)) = super::tikz::svg(&super::tikz::com_definicoes(corpo, &self.ctx.tikz), &cor) else {
            self.ctx.avisos.ambiente("tikzpicture");
            return;
        };
        if faltou {
            self.ctx.avisos.anotar(TipoDeAviso::SemEfeito, "tikzpicture");
        }
        // As cores do preâmbulo que o desenho usa vão junto (o `.tex` exportado não tem o preâmbulo original).
        let mut fonte: String = self.ctx.cores.iter().filter(|(n, _)| corpo.contains(n.as_str())).map(|(n, c)| format!("\\definecolor{{{n}}}{{HTML}}{{{:06X}}}\n", (c >> 8) & 0xFFFFFF)).collect();
        fonte.push_str(&format!("\\begin{{tikzpicture}}{}\n\\end{{tikzpicture}}", corpo.trim_end()));
        let svg = com_fonte_tikz(&svg, &fonte);
        let origem = crate::base64::para_data_url("image/svg+xml", svg.as_bytes());
        self.push_bloco(Conteudo::Imagem { origem, legenda: Vec::new(), largura, altura });
    }

    /// `description`: cada `\item[termo] texto` é um parágrafo com o termo em negrito.
    pub(super) fn descricao(&mut self, corpo: &str) {
        let (_, itens) = super::itens_de(corpo);
        for item in itens {
            // O termo é o `[…]` logo depois do `\item` que abre este item.
            let ini = item.as_ptr() as usize - corpo.as_ptr() as usize;
            let termo = corpo[..ini].rfind("\\item").and_then(|p| opcional(&corpo[p + 5..ini])).map(|(t, _)| t);
            let mut cabeca: Vec<Trecho> = termo.map(|t| self.inline_aqui(t)).unwrap_or_default();
            cabeca.iter_mut().for_each(|t| t.estilo |= NEGRITO);
            if !cabeca.is_empty() {
                cabeca.push(Trecho { texto: " ".into(), estilo: self.ctx.classe.familia, ..Trecho::default() });
            }
            self.com_cabeca(cabeca, item, self.ctx.clone(), None);
        }
    }
}

/// O `tikzpicture` (com as `\definecolor` que ele usa) guardado no SVG (`<desc id="tikz">`, em base64): a exportação para
/// LaTeX devolve o desenho como estava, que o pdfLaTeX compila (SVG ele não lê).
fn com_fonte_tikz(svg: &str, corpo: &str) -> String {
    let desc = format!("<desc id=\"tikz\">{}</desc>", crate::base64::codificar(corpo.as_bytes()));
    match svg.find("<svg").and_then(|i| svg[i..].find('>').map(|k| i + k + 1)) {
        Some(p) => format!("{}{desc}{}", &svg[..p], &svg[p..]),
        None => svg.to_string(),
    }
}

/// O código do `tikzpicture` que veio no SVG da imagem, se veio.
pub fn fonte_tikz(origem: &str) -> Option<String> {
    let (_, bytes) = crate::base64::de_data_url(origem)?;
    let svg = String::from_utf8(bytes).ok()?;
    let ini = svg.find("<desc id=\"tikz\">")? + "<desc id=\"tikz\">".len();
    let fim = ini + svg[ini..].find("</desc>")?;
    String::from_utf8(crate::base64::decodificar(&svg[ini..fim])?).ok()
}
