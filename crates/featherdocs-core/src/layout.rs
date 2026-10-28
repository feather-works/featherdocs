//! A composição de **um bloco**: quebra de linha e alturas, sem saber em que
//! página ele cai (isso é da `paginacao`). O resultado de um bloco só depende
//! do conteúdo dele e da largura útil, por isso pode ser guardado e reaproveitado
//! enquanto o bloco não muda — é o que torna a edição barata em documento grande.

use crate::inline::texto_puro;
use crate::medida::{largura, largura_do_caractere};
use crate::modelo::{Alinhamento, Atrib, Celula, Conteudo, Estilo, InicioDeSecao, Lista, Marca as Objeto, MesclagemTabela, Trecho, CODIGO, DISTRIBUIDO, LINHAS_JUNTAS, MAIUSCULAS, MANTER_COM_PROXIMO, QUEBRA_ANTES, QUEBRA_DE_LINHA, EXIBIDA, ITALICO, MATEMATICA, NEGRITO, SERIFA, SUBLINHADO, TACHADO_DUPLO, TIPOS_DE_SUBLINHADO, VERSALETE, VIUVAS_LIVRES};

pub const COR_TEXTO: u32 = 0x1a1a_1aff;
pub const COR_LINK: u32 = 0x1a56_c4ff;
pub const COR_CITACAO: u32 = 0x5555_55ff;
pub const COR_CINZA: u32 = 0x8086_8bff;
pub const COR_FUNDO_CODIGO: u32 = 0xf1f3_f4ff;
pub const COR_BORDA: u32 = 0xbdc1_c6ff;

/// O fundo do trecho comentado.
pub const COR_COMENTADO: u32 = 0xfff0_b3ff;

/// A cor das sugestões de cada autor (o mesmo nome dá sempre a mesma cor).
pub fn cor_do_autor(autor: &str) -> u32 {
    const CORES: [u32; 6] = [0xc522_1fff, 0x1a73_e8ff, 0x1880_38ff, 0x9334_e6ff, 0xe374_00ff, 0x007b_83ff];
    let h = autor.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    CORES[(h % 6) as usize]
}

/// Corpo do texto, em pontos, e a entrelinha (múltiplo do corpo).
pub const CORPO: f32 = 11.0;
const ENTRELINHA: f32 = 1.4;
const TAMANHOS_TITULO: [f32; 6] = [24.0, 18.0, 15.0, 13.0, 12.0, 11.0];

/// Um pedaço de texto já posicionado: `x` na linha, `y` é a linha de base abaixo do topo da linha.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pedaco {
    pub x: f32,
    pub y: f32,
    pub texto: String,
    pub estilo: u8,
    pub tamanho: f32,
    pub cor: u32,
    /// Posição (em caracteres) do primeiro caractere deste pedaço no texto do parágrafo ou da célula.
    pub de: usize,
    /// Numa tabela, a coluna da célula a que o pedaço pertence.
    pub celula: Option<u16>,
    /// Cor do marca-texto atrás do pedaço (`0xRRGGBBAA`); 0 = sem.
    pub realce: u32,
    /// Quanto o texto sobe (+) ou desce (−) em relação à linha de base (sobrescrito/subscrito).
    pub desloc: f32,
    /// Um objeto preso ao texto (nota, citação, campo): o `texto` é o rótulo dele, mas para o cursor é
    /// **um caractere só**.
    pub atomo: bool,
    /// Os `MaisLetra::efeitos` e o tipo do sublinhado: o motor põe os traços que não são o simples
    /// (duplo, ondulado, tachado duplo) como `Deco::Traco` da linha.
    pub efeitos: u8,
    pub sublinhado: u8,
}

impl Pedaco {
    /// Quantos caracteres do texto do parágrafo o pedaço ocupa (o objeto conta um).
    pub fn caracteres(&self) -> usize {
        if self.atomo {
            1
        } else {
            self.texto.chars().count()
        }
    }
}

/// Uma nota (de rodapé ou de fim) já composta, presa à linha onde está a chamada dela.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NotaComposta {
    pub fim: bool,
    pub linhas: Vec<Linha>,
    /// Onde está a chamada: o caractere no texto do parágrafo (ou da célula) e, em tabela, a coluna.
    pub desl: usize,
    pub coluna: Option<u16>,
}

impl NotaComposta {
    /// A altura que a nota ocupa no pé da página (com um respiro embaixo).
    pub fn altura(&self) -> f32 {
        self.linhas.iter().map(|l| l.altura).sum::<f32>() + 2.0
    }
}

/// Onde o texto de uma célula de tabela começa na linha: para o cursor numa célula vazia.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InfoCelula {
    pub x0: f32,
    pub base: f32,
    pub tam: f32,
}

/// Algo que não é texto: fundo, traço, caixa de imagem (`y` relativo ao topo da linha).
#[derive(Clone, Debug, PartialEq)]
pub enum Deco {
    Retangulo { x: f32, y: f32, w: f32, h: f32, cor: u32 },
    Traco { x1: f32, y1: f32, x2: f32, y2: f32, espessura: f32, cor: u32 },
    Imagem { x: f32, y: f32, w: f32, h: f32, origem: String },
    /// Texto que não é do documento (o número de uma equação): o cursor não passa por ele. `y` = linha de base.
    Texto { x: f32, y: f32, texto: String, tamanho: f32, estilo: u8, cor: u32 },
}

/// Uma linha composta; é a menor unidade que a paginação separa.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Linha {
    pub altura: f32,
    pub pedacos: Vec<Pedaco>,
    pub deco: Vec<Deco>,
    /// Entrada de sumário: o título para onde ela aponta (o número da página sai no quadro).
    pub alvo: Option<u64>,
    /// Na entrada de sumário: 1 = com o número da página, 2 = com o pontilhado até ele, 4 = no jeito do LaTeX.
    pub sumario: u8,
    /// Posição do primeiro caractere desta linha no texto do parágrafo (para achar o cursor).
    pub de: usize,
    /// Onde o texto começa (recuo) e a linha de base e o corpo, para o cursor em linha vazia.
    pub x0: f32,
    pub base: f32,
    pub tam: f32,
    /// Só em linha de tabela: uma entrada por coluna.
    pub celulas: Vec<InfoCelula>,
    /// As notas cujas chamadas estão nesta linha, na ordem (a paginação as põe no pé da mesma página).
    pub notas: Vec<NotaComposta>,
    /// Os comentários que começam nesta linha (o quadro põe um balão na margem direita).
    pub comentarios: Vec<u64>,
}

/// O bloco composto, com as regras de quebra de página dele.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutBloco {
    pub linhas: Vec<Linha>,
    pub antes: f32,
    pub depois: f32,
    /// Quantas linhas, no mínimo, ficam no fim de uma página antes de quebrar (órfã).
    pub minimo_antes: usize,
    /// E quantas vão para a página seguinte (viúva).
    pub minimo_depois: usize,
    pub manter_com_proximo: bool,
    pub quebra_antes: bool,
    /// Posição `x` do marcador de lista (•, 1.), se for item.
    pub marcador_x: Option<f32>,
    /// Depois deste bloco, o que vem começa uma página nova (quebra de página).
    pub quebra_depois: bool,
    /// Só aparece na tela (a marca "quebra de página"): o PDF e a impressão pulam.
    pub so_na_tela: bool,
    /// Com `quebra_depois`: a página nova tem de ser par (`true`) ou ímpar; se não for, fica uma em branco no meio.
    pub pagina_par: Option<bool>,
    /// O número automático do título, desenhado antes da primeira linha (fora do texto: o cursor não passa por ele).
    pub numero: Option<NumeroDoTitulo>,
    /// A cor do marcador de lista; 0 = a do texto.
    pub cor_marcador: u32,
    /// O estilo do marcador além da família do texto (`NEGRITO` no `\textbf{Art. 1º}` do LaTeX).
    pub estilo_marcador: u8,
    /// Espaço elástico antes (`Atrib::encher`).
    pub encher: bool,
}

/// O número de um título ("1.2"), com a letra do próprio título.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NumeroDoTitulo {
    pub texto: String,
    pub x: f32,
    pub tamanho: f32,
    pub estilo: u8,
    pub cor: u32,
}

/// O que vem de fora do bloco e muda a composição dele: o número do título e o estilo com nome.
#[derive(Clone, Copy, Debug, Default)]
pub struct Entorno<'a> {
    pub numero: Option<&'a str>,
    pub estilo: Option<&'a Estilo>,
}

/// O que decide como um pedaço de texto se desenha: igual em fichas vizinhas = mesmo pedaço.
#[derive(Clone, Copy, PartialEq)]
struct Marca {
    estilo: u8,
    cor: u32,
    tamanho: f32,
    realce: u32,
    /// Quanto a linha de base sobe (+) ou desce (−) por causa de sobrescrito/subscrito, em pontos.
    desloc: f32,
    efeitos: u8,
    sublinhado: u8,
}

/// Uma palavra ou um espaço, com a largura já medida.
#[derive(Clone)]
struct Ficha {
    texto: String,
    marca: Marca,
    largura: f32,
    espaco: bool,
    /// Tabulação (`\t`): o que vem depois dela na linha encosta na margem direita (o `\hfill` do LaTeX).
    tab: bool,
    /// Posição do primeiro caractere no texto do parágrafo.
    de: usize,
    /// Objeto preso ao texto: o `texto` é o rótulo, e ele ocupa um caractere.
    atomo: bool,
    /// Fórmula em linha (`$…$`): uma ficha só, que não se parte nem se junta com a vizinha.
    formula: bool,
    /// Quebra de linha forçada ([`QUEBRA_DE_LINHA`]): termina a linha nela.
    quebra: bool,
    /// Pedaço de palavra partida no fim da linha: ganha o hífen desenhado depois dele.
    hifen: bool,
}

/// Onde o texto de um bloco começa, o quanto cabe, e como medir o tipo de bloco.
#[derive(Clone, Copy)]
struct Base {
    tamanho: f32,
    extra: u8,
    cor: u32,
    x0: f32,
    limite: f32,
    /// `false` em célula de tabela: o tamanho e a posição do trecho não valem (a altura da linha é fixa).
    usar_tamanho: bool,
    /// A largura da área de texto da página (onde as notas de rodapé se compõem); 0 = `limite`.
    pagina: f32,
    /// Os `MaisLetra::efeitos` do estilo do bloco (versalete num título).
    efeitos: u8,
}

impl Base {
    fn novo(tamanho: f32, extra: u8, cor: u32, x0: f32, limite: f32) -> Self {
        Self { tamanho, extra, cor, x0, limite, usar_tamanho: true, pagina: 0.0, efeitos: 0 }
    }

    fn largura_da_pagina(&self) -> f32 {
        if self.pagina > 0.0 {
            self.pagina
        } else {
            self.x0 + self.limite
        }
    }
}

fn estilo_efetivo(t: &Trecho, extra: u8) -> u8 {
    let mut e = t.estilo | extra;
    // Link com cor própria (o `colorlinks` do LaTeX) só pinta; sem cor, o azul sublinhado de sempre.
    if t.link.is_some() && t.fmt.cor == 0 {
        e |= SUBLINHADO;
    }
    e
}

/// Como o trecho se desenha neste bloco: tamanho, cor, realce e deslocamento de sobrescrito/subscrito.
fn marca_do_trecho(t: &Trecho, b: &Base) -> Marca {
    let mut tamanho = b.tamanho;
    let mut desloc = 0.0;
    let posicao = if matches!(t.marca.as_deref(), Some(Objeto::Nota { .. })) { 1 } else { t.fmt.posicao };
    if b.usar_tamanho || posicao > 0 && t.marca.is_some() {
        if t.fmt.tamanho > 0.0 && b.usar_tamanho {
            tamanho = t.fmt.tamanho;
        }
        if posicao != 0 {
            desloc = tamanho * if posicao > 0 { 0.35 } else { -0.14 };
            tamanho *= 0.65;
        }
    }
    let efeitos = t.fmt.mais.efeitos | b.efeitos;
    // shortcut: o versalete encolhe a palavra toda (as maiúsculas do original também); partir a ficha por caixa se incomodar.
    if efeitos & VERSALETE != 0 && efeitos & MAIUSCULAS == 0 && t.texto.chars().any(char::is_lowercase) {
        tamanho *= 0.8;
    }
    desloc += t.fmt.mais.elevacao;
    let cor = if let Some(r) = t.rev.as_deref() {
        cor_do_autor(&r.autor)
    } else if t.link.is_some() && t.fmt.cor == 0 {
        COR_LINK
    } else if t.fmt.cor != 0 {
        t.fmt.cor
    } else {
        b.cor
    };
    let mut estilo = estilo_efetivo(t, b.extra);
    match t.rev.as_deref().map(|r| r.tipo) {
        Some(crate::modelo::TipoDeRevisao::Insercao) => estilo |= SUBLINHADO,
        Some(crate::modelo::TipoDeRevisao::Remocao) => estilo |= crate::modelo::TACHADO,
        None => {}
    }
    Marca { estilo, cor, tamanho, realce: t.fmt.realce, desloc, efeitos, sublinhado: t.fmt.mais.sublinhado }
}

/// Parte os trechos em palavras e espaços.
fn fichas(trechos: &[Trecho], b: &Base) -> Vec<Ficha> {
    let mut saida = Vec::new();
    let mut de = 0;
    // Os comentários abertos neste ponto do texto: o trecho deles ganha fundo.
    let mut comentados = 0usize;
    for t in trechos {
        let mut marca = marca_do_trecho(t, b);
        if let Some(Objeto::Comentario { fim, .. }) = t.marca.as_deref() {
            comentados = if *fim { comentados.saturating_sub(1) } else { comentados + 1 };
        } else if comentados > 0 && marca.realce == 0 {
            marca.realce = COR_COMENTADO;
        }
        if let Some(Objeto::Imagem { largura, .. }) = t.marca.as_deref() {
            saida.push(Ficha { atomo: true, largura: *largura, ..ficha("", marca, false, de) });
            de += 1;
            continue;
        }
        if t.marca.is_some() {
            saida.push(Ficha { atomo: true, ..ficha(&texto_do_objeto(t), marca, false, de) });
            de += 1;
            continue;
        }
        if marca.estilo & MATEMATICA != 0 && !t.texto.is_empty() {
            saida.push(Ficha { formula: true, ..ficha(&t.texto, marca, false, de) });
            de += t.texto.chars().count();
            continue;
        }
        let mut atual = String::new();
        let mut inicio = de;
        let mut espaco = false;
        // A palavra que começa com "http" ou "www." é um endereço, mesmo sem link.
        let mut endereco = false;
        for c in t.texto.chars() {
            if c == '\t' {
                if !atual.is_empty() {
                    saida.push(ficha(&atual, marca, espaco, inicio));
                    atual.clear();
                }
                saida.push(Ficha { texto: " ".into(), marca, largura: 0.0, espaco: false, tab: true, de, atomo: false, formula: false, quebra: false, hifen: false });
                de += 1;
                inicio = de;
                espaco = false;
                continue;
            }
            if c == QUEBRA_DE_LINHA {
                if !atual.is_empty() {
                    saida.push(ficha(&atual, marca, espaco, inicio));
                    atual.clear();
                }
                saida.push(Ficha { texto: " ".into(), marca, largura: 0.0, espaco: true, tab: false, de, atomo: false, formula: false, quebra: true, hifen: false });
                de += 1;
                inicio = de;
                espaco = false;
                continue;
            }
            let c = if c == '\n' { ' ' } else { maiuscula(c, marca.efeitos) };
            if !atual.is_empty() && (c == ' ') != espaco {
                saida.push(ficha(&atual, marca, espaco, inicio));
                atual.clear();
                inicio = de;
            }
            espaco = c == ' ';
            atual.push(c);
            de += 1;
            endereco = !espaco && (endereco || atual == "http" || atual == "www.");
            // Como o pacote url: um endereço pode quebrar a linha depois destes sinais.
            if (t.link.is_some() || endereco) && matches!(c, '/' | ':' | '.' | '?' | '&' | '=' | '#' | '_') {
                saida.push(ficha(&atual, marca, false, inicio));
                atual.clear();
                inicio = de;
            }
        }
        if !atual.is_empty() {
            saida.push(ficha(&atual, marca, espaco, inicio));
        }
    }
    saida
}

/// A letra em maiúscula no texto em versalete ou todo em maiúsculas (só quando ela continua uma letra
/// só, para o cursor não se perder: o "ß" fica).
fn maiuscula(c: char, efeitos: u8) -> char {
    if efeitos & (MAIUSCULAS | VERSALETE) == 0 {
        return c;
    }
    let mut m = c.to_uppercase();
    match (m.next(), m.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

fn ficha(texto: &str, marca: Marca, espaco: bool, de: usize) -> Ficha {
    Ficha { texto: texto.to_string(), marca, largura: largura(texto, marca.estilo, marca.tamanho), espaco, tab: false, de, atomo: false, formula: false, quebra: false, hifen: false }
}

/// O que um objeto preso ao texto mostra: o rótulo que o motor calculou. O campo de página, que só se
/// sabe no quadro, reserva o espaço de dois algarismos.
fn texto_do_objeto(t: &Trecho) -> String {
    t.rotulo.clone()
}

/// Uma palavra maior que a linha inteira é partida por caracteres.
fn partir_palavra(f: &Ficha, limite: f32) -> Vec<Ficha> {
    let mut partes = Vec::new();
    let mut atual = String::new();
    let mut inicio = f.de;
    let mut w = 0.0;
    for (passou, c) in f.texto.chars().enumerate() {
        let cw = f32::from(largura_do_caractere(c, f.marca.estilo)) * f.marca.tamanho / 1000.0;
        if w + cw > limite && !atual.is_empty() {
            partes.push(ficha(&atual, f.marca, false, inicio));
            atual.clear();
            inicio = f.de + passou;
            w = 0.0;
        }
        atual.push(c);
        w += cw;
    }
    if !atual.is_empty() {
        partes.push(ficha(&atual, f.marca, false, inicio));
    }
    partes
}

/// A palavra que não cabe no fim de uma linha justificada: o pedaço (até a sílaba) que cabe com o
/// hífen e o resto. Como o TeX, só parte quando a linha sem ela esticaria os espaços além do normal
/// (metade da largura do espaço em cada um).
fn hifenizar(f: &Ficha, linha: &[Ficha], sobra: f32) -> Option<(Ficha, Ficha)> {
    if f.espaco || f.tab || f.atomo || f.formula || f.quebra || f.marca.estilo & CODIGO != 0 {
        return None;
    }
    let fim = linha.iter().rposition(|x| !x.espaco)?;
    let espacos: Vec<&Ficha> = linha[..fim].iter().filter(|x| x.espaco).collect();
    let folga = sobra + linha[fim + 1..].iter().map(|x| x.largura).sum::<f32>();
    if !espacos.is_empty() && folga <= espacos.iter().map(|x| x.largura).sum::<f32>() * 0.5 {
        return None;
    }
    let hifen = largura("-", f.marca.estilo, f.marca.tamanho);
    let cs: Vec<char> = f.texto.chars().collect();
    let k = crate::silabas::pontos(&f.texto).into_iter().rev().find(|&k| largura(&cs[..k].iter().collect::<String>(), f.marca.estilo, f.marca.tamanho) + hifen <= sobra)?;
    let antes = Ficha { hifen: true, ..ficha(&cs[..k].iter().collect::<String>(), f.marca, false, f.de) };
    Some((antes, ficha(&cs[k..].iter().collect::<String>(), f.marca, false, f.de + k)))
}

#[path = "quebra_tex.rs"]
mod quebra_tex;

thread_local! {
    /// O documento em composição veio do LaTeX (`Extras::composicao_tex`): o justificado quebra como o TeX.
    static COMPOSICAO_TEX: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Liga a quebra do TeX no justificado para o que se compuser daqui em diante nesta thread.
pub fn composicao_tex(sim: bool) {
    COMPOSICAO_TEX.with(|c| c.set(sim));
}

/// As linhas do parágrafo: no justificado de um documento do LaTeX, o parágrafo inteiro de uma vez
/// (cada trecho entre quebras forçadas); senão, ou se nada cabe na tolerância, linha a linha.
fn quebrar(fichas: Vec<Ficha>, limite_geral: f32, primeira: f32, justificado: bool) -> Vec<Vec<Ficha>> {
    if !justificado || !COMPOSICAO_TEX.with(|c| c.get()) || fichas.iter().any(|f| f.tab) {
        return quebrar_linha_a_linha(fichas, limite_geral, primeira, justificado);
    }
    let mut linhas: Vec<Vec<Ficha>> = Vec::new();
    let mut trecho: Vec<Ficha> = Vec::new();
    let n = fichas.len();
    for (k, f) in fichas.iter().cloned().enumerate() {
        let quebra = f.quebra;
        if !quebra {
            trecho.push(f.clone());
        }
        if quebra || k + 1 == n {
            let largura_primeira = if linhas.is_empty() { (limite_geral - primeira).max(1.0) } else { limite_geral };
            let Some(mut ls) = quebra_tex::quebrar(&mut trecho, largura_primeira, limite_geral) else {
                return quebrar_linha_a_linha(fichas, limite_geral, primeira, justificado);
            };
            if quebra {
                match ls.last_mut() {
                    Some(l) => l.push(f),
                    None => ls.push(vec![f]),
                }
            }
            linhas.extend(ls);
            trecho.clear();
        }
    }
    linhas
}

/// Quebra gulosa: enche a linha até não caber. Devolve as linhas como listas de fichas.
/// `limite_geral` vale para as linhas de continuação; a primeira tem `limite_geral - primeira` (recuo de primeira linha).
fn quebrar_linha_a_linha(fichas: Vec<Ficha>, limite_geral: f32, primeira: f32, justificado: bool) -> Vec<Vec<Ficha>> {
    let mut linhas: Vec<Vec<Ficha>> = Vec::new();
    let mut atual: Vec<Ficha> = Vec::new();
    let mut w = 0.0;
    // O que vem depois de cada ficha até a quebra forçada (para a tabulação saber se o resto cabe na linha).
    let mut depois: Vec<f32> = fichas.iter().rev().scan(0.0, |soma, f| {
        if f.quebra {
            *soma = 0.0;
        }
        let antes = *soma;
        *soma += f.largura;
        Some(antes)
    }).collect();
    depois.reverse();
    for (k, mut f) in fichas.into_iter().enumerate() {
        let limite = if linhas.is_empty() { (limite_geral - primeira).max(1.0) } else { limite_geral };
        // Tabulação: o que vem depois dela vai inteiro para a direita; se não cabe, desce junto para a linha seguinte.
        if f.tab && w + depois[k] > limite && !atual.is_empty() {
            linhas.push(std::mem::take(&mut atual));
            w = 0.0;
        }
        if f.quebra {
            atual.push(f);
            linhas.push(std::mem::take(&mut atual));
            w = 0.0;
            continue;
        }
        if f.espaco {
            // Espaço no começo de uma linha de continuação some; no começo do parágrafo fica.
            if !atual.is_empty() || linhas.is_empty() {
                w += f.largura;
                atual.push(f);
            }
            continue;
        }
        // Justificado, como o TeX: os espaços da linha encolhem até 1/3 para caber mais uma palavra.
        let encolhe = |atual: &[Ficha]| if justificado { atual.iter().filter(|x| x.espaco).map(|x| x.largura).sum::<f32>() / 3.0 } else { 0.0 };
        if w + f.largura > limite + encolhe(&atual) && !atual.is_empty() {
            if justificado {
                if let Some((antes, resto)) = hifenizar(&f, &atual, limite - w) {
                    atual.push(antes);
                    f = resto;
                }
            }
            linhas.push(std::mem::take(&mut atual));
            w = 0.0;
        }
        let pecas = if f.largura > limite && !f.atomo && !f.formula { partir_palavra(&f, limite) } else { vec![f] };
        for p in pecas {
            if w + p.largura > limite + encolhe(&atual) && !atual.is_empty() {
                linhas.push(std::mem::take(&mut atual));
                w = 0.0;
            }
            w += p.largura;
            atual.push(p);
        }
    }
    if !atual.is_empty() {
        linhas.push(atual);
    }
    linhas
}

/// Junta fichas vizinhas de mesma marca num pedaço só, sem o espaço do fim. Depois de uma tabulação,
/// o resto da linha encosta em `direita`.
fn pedacos_da_linha(mut fichas: Vec<Ficha>, x0: f32, y: f32, ultima: bool, justificar: Option<f32>, direita: f32) -> Vec<Pedaco> {
    // O espaço do fim some quando a linha quebra; na última linha fica (o cursor anda ao digitar espaço).
    while !ultima && fichas.last().is_some_and(|f| f.espaco) {
        fichas.pop();
    }
    let mut saida: Vec<Pedaco> = Vec::new();
    let mut x = x0;
    // Justificado: o que sobra na linha é repartido entre os espaços; cada palavra vira um pedaço.
    let extra = justificar.map_or(0.0, |sobra| {
        let espacos = fichas.iter().rposition(|f| !f.espaco).map_or(0, |u| fichas[..u].iter().filter(|f| f.espaco).count());
        if espacos == 0 {
            0.0
        } else {
            sobra / espacos as f32
        }
    });
    let partir = justificar.is_some() && extra != 0.0;
    let larguras: Vec<f32> = fichas.iter().map(|f| f.largura).collect();
    for (k, f) in fichas.into_iter().enumerate() {
        if f.tab {
            let resto: f32 = larguras[k + 1..].iter().sum();
            x = x.max(direita - resto);
            // O pedaço começa no caractere depois da tabulação: o texto que vier junta nele, já encostado à direita.
            saida.push(Pedaco { x, y, texto: String::new(), estilo: f.marca.estilo, tamanho: f.marca.tamanho, cor: f.marca.cor, de: f.de + 1, realce: f.marca.realce, desloc: f.marca.desloc, celula: None, atomo: false, efeitos: f.marca.efeitos, sublinhado: f.marca.sublinhado });
            continue;
        }
        let w = f.largura;
        let (espaco, m, atomo) = (f.espaco, f.marca, f.atomo);
        let formula = f.formula || saida.last().is_some_and(|p| p.estilo & MATEMATICA != 0);
        match saida.last_mut() {
            Some(p) if !partir && !atomo && !p.atomo && !formula && p.estilo == m.estilo && p.cor == m.cor && p.tamanho == m.tamanho && p.realce == m.realce && p.desloc == m.desloc && (p.efeitos, p.sublinhado) == (m.efeitos, m.sublinhado) => p.texto.push_str(&f.texto),
            _ => saida.push(Pedaco { x, y, texto: f.texto, estilo: m.estilo, tamanho: m.tamanho, cor: m.cor, de: f.de, celula: None, realce: m.realce, desloc: m.desloc, atomo, efeitos: m.efeitos, sublinhado: m.sublinhado }),
        }
        x += w;
        if partir && espaco {
            x += extra;
        }
    }
    saida
}

/// As imagens presas ao texto da linha, em pé na linha de base; a mais alta que o texto empurra a linha
/// para baixo.
fn imagens_na_linha(trechos: &[Trecho], altura: &mut f32, base: &mut f32, pedacos: &mut [Pedaco]) -> Vec<Deco> {
    let imagens: Vec<(f32, &str, f32, f32)> = pedacos
        .iter()
        .filter(|p| p.atomo)
        .filter_map(|p| match crate::texto::objeto_em(trechos, p.de) {
            Some(Objeto::Imagem { origem, largura, altura }) => Some((p.x, origem.as_str(), *largura, *altura)),
            _ => None,
        })
        .collect();
    let falta = imagens.iter().map(|i| i.3 - *base).fold(0.0, f32::max);
    if falta > 0.0 {
        *altura += falta;
        *base += falta;
        pedacos.iter_mut().for_each(|p| p.y += falta);
    }
    imagens.into_iter().map(|(x, origem, w, h)| Deco::Imagem { x, y: *base - h, w, h, origem: origem.to_string() }).collect()
}

/// A linha de base dentro de uma linha de altura `altura` para texto de `tamanho`.
fn linha_de_base(altura: f32, tamanho: f32) -> f32 {
    (altura - tamanho) / 2.0 + tamanho * 0.8
}

/// Compõe um texto corrido em linhas.
fn texto_corrido(trechos: &[Trecho], tamanho: f32, extra: u8, cor: u32, x0: f32, limite: f32) -> Vec<Linha> {
    texto_formatado(trechos, &Base::novo(tamanho, extra, cor, x0, limite), &Atrib::default())
}

/// O texto de uma célula de tabela: sem tamanho próprio, porque a altura da linha da tabela é fixa
/// (a do LaTeX, `tamanhos`, mostra o `{\small …}` da célula: a linha continua com a altura do corpo).
fn texto_celula(trechos: &[Trecho], tamanho: f32, extra: u8, limite: f32, pagina: f32, tamanhos: bool) -> Vec<Linha> {
    let b = Base { usar_tamanho: tamanhos, pagina, ..Base::novo(tamanho, extra, COR_TEXTO, 0.0, limite) };
    texto_formatado(trechos, &b, &Atrib::default())
}

/// A largura do que a linha mostra, sem os espaços do fim (que não entram no alinhamento).
fn largura_visivel(fichas: &[Ficha]) -> f32 {
    let fim = fichas.iter().rposition(|f| !f.espaco).map_or(0, |u| u + 1);
    fichas[..fim].iter().map(|f| f.largura).sum()
}

/// A entrelinha de um parágrafo como múltiplo do padrão (1,15).
/// A altura da linha em pontos inteiros; no documento do LaTeX, a exata (o `\baselineskip` de 13,6 pt
/// do corpo 11 vira 14 arredondado, e a diferença soma uma linha a cada 35).
fn altura_da_linha(h: f32) -> f32 {
    if COMPOSICAO_TEX.with(|c| c.get()) {
        h
    } else {
        h.round()
    }
}

fn fator_entrelinha(a: &Atrib) -> f32 {
    if a.entrelinha > 0.0 {
        a.entrelinha / 1.15
    } else {
        1.0
    }
}

/// A altura da linha com a entrelinha exata ou "pelo menos" do Word, se o parágrafo tiver.
fn altura_fixa(a: &Atrib, altura: f32) -> f32 {
    if a.mais.entrelinha_exata > 0.0 {
        a.mais.entrelinha_exata
    } else {
        altura.max(a.mais.entrelinha_minima)
    }
}

/// Os traços que a tela não desenha sozinha pelo bit `SUBLINHADO`/`TACHADO`: o sublinhado duplo
/// (um segundo traço embaixo), o ondulado (em zigue-zague, no lugar do reto) e o tachado duplo.
// shortcut: os outros tipos (pontilhado, tracejado, grosso) saem como o simples; desenhar quando alguém pedir.
fn tracos_especiais(pedacos: &mut [Pedaco], deco: &mut Vec<Deco>) {
    for p in pedacos.iter_mut().filter(|p| !p.texto.is_empty()) {
        let tipo = TIPOS_DE_SUBLINHADO.get(usize::from(p.sublinhado)).copied().unwrap_or("single");
        let duplo = p.efeitos & TACHADO_DUPLO != 0;
        let sub = p.estilo & SUBLINHADO != 0 && matches!(tipo, "double" | "wave" | "wavyHeavy" | "wavyDouble");
        if !duplo && !sub {
            continue;
        }
        let (x1, x2, y, t) = (p.x, p.x + largura(&p.texto, p.estilo, p.tamanho), p.y - p.desloc, p.tamanho);
        let espessura = (t / 16.0).max(0.5);
        let mut traco = |y1: f32, y2: f32, xa: f32, xb: f32| deco.push(Deco::Traco { x1: xa, y1, x2: xb, y2, espessura, cor: p.cor });
        if duplo {
            for dy in [-0.36, -0.24] {
                traco(y + t * dy, y + t * dy, x1, x2);
            }
        }
        if sub && tipo == "double" {
            traco(y + t * 0.12 + 2.0 * espessura, y + t * 0.12 + 2.0 * espessura, x1, x2);
        } else if sub {
            // A tela não põe o reto: a onda vai no lugar dele.
            p.estilo &= !SUBLINHADO;
            let (passo, amplitude) = (t * 0.2, t * 0.05);
            let mut x = x1;
            let mut cima = true;
            while x < x2 {
                let fim = (x + passo).min(x2);
                let (ya, yb) = if cima { (y + t * 0.12 + amplitude, y + t * 0.12 - amplitude) } else { (y + t * 0.12 - amplitude, y + t * 0.12 + amplitude) };
                traco(ya, yb, x, fim);
                x = fim;
                cima = !cima;
            }
        }
    }
}

/// O maior corpo entre o que a linha mostra (o do bloco, se só há espaço).
fn corpo_da_linha(f: &[Ficha], padrao: f32) -> f32 {
    f.iter().filter(|x| !x.espaco).map(|x| x.marca.tamanho).fold(0.0, f32::max).max(if f.iter().any(|x| !x.espaco) { 0.0 } else { padrao })
}

/// Como `texto_corrido`, com o alinhamento, os recuos e a entrelinha do parágrafo. `b.x0`/`b.limite` já
/// descontam o que o próprio tipo de bloco reserva (marcador de lista, barra da citação); `a.recuo_esq` e
/// `a.recuo_dir` entram por cima.
fn texto_formatado(trechos: &[Trecho], b: &Base, a: &Atrib) -> Vec<Linha> {
    let esq = b.x0 + a.recuo_esq.max(0.0);
    let util = (b.limite - a.recuo_esq.max(0.0) - a.recuo_dir.max(0.0)).max(1.0);
    // A primeira linha pode começar antes (negativo) da margem do parágrafo, até a borda esquerda.
    let primeira = a.primeira.max(-esq);
    let quebradas = quebrar(fichas(trechos, b), util, primeira, a.alinhamento == Alinhamento::Justificado);
    let total = quebradas.len();
    let fator = fator_entrelinha(a);
    let mut linhas: Vec<Linha> = quebradas
        .into_iter()
        .enumerate()
        .map(|(k, f)| {
            // O `\hfill\\` do LaTeX (tabulação e quebra): a linha não estica, como a última.
            let hfil = f.iter().rev().skip_while(|x| x.quebra || x.espaco && !x.tab).next().is_some_and(|x| x.tab) && f.last().is_some_and(|x| x.quebra);
            let ultima = k + 1 == total || hfil;
            let disponivel = if k == 0 { util - primeira } else { util };
            let inicio = if k == 0 { esq + primeira } else { esq };
            // A palavra partida: o hífen entra na largura da linha e é desenhado depois do último pedaço.
            let hifen = f.iter().rev().find(|x| !x.espaco).filter(|x| x.hifen).map(|x| (x.marca, largura("-", x.marca.estilo, x.marca.tamanho)));
            let visivel = largura_visivel(&f) + hifen.map_or(0.0, |h| h.1);
            let (x_ini, justificar) = match a.alinhamento {
                Alinhamento::Esquerda => (inicio, None),
                Alinhamento::Centro => (inicio + ((disponivel - visivel) / 2.0).max(0.0), None),
                Alinhamento::Direita => (inicio + (disponivel - visivel).max(0.0), None),
                // A linha que só coube com os espaços encolhidos encolhe mesmo sendo a última.
                Alinhamento::Justificado => (inicio, (!ultima || visivel > disponivel || a.mais.fluxo & DISTRIBUIDO != 0).then_some(disponivel - visivel)),
            };
            let tam = corpo_da_linha(&f, b.tamanho);
            let mut altura = altura_fixa(a, altura_da_linha(tam * ENTRELINHA * fator));
            let mut base = linha_de_base(altura, tam);
            let mut pedacos = pedacos_da_linha(f, x_ini, base, ultima, justificar, inicio + disponivel);
            if b.usar_tamanho {
                acomodar_formulas(&mut altura, &mut base, &mut pedacos);
            }
            let mut deco = imagens_na_linha(trechos, &mut altura, &mut base, &mut pedacos);
            tracos_especiais(&mut pedacos, &mut deco);
            if let (Some((m, _)), Some(p)) = (hifen, pedacos.last()) {
                deco.push(Deco::Texto { x: p.x + largura(&p.texto, p.estilo, p.tamanho), y: base - m.desloc, texto: "-".into(), tamanho: m.tamanho, estilo: m.estilo, cor: m.cor });
            }
            Linha { altura, de: pedacos.first().map_or(0, |p| p.de), x0: x_ini, base, tam, pedacos, deco, ..Linha::default() }
        })
        .collect();
    if linhas.is_empty() {
        linhas.push(linha_vazia(b, a, esq + primeira, util - primeira));
    }
    // A folga do fundo antes das bordas: a de baixo fica embaixo do texto, não em cima dele.
    if a.fundo != 0 {
        // Na caixa do `tcolorbox`, 2 mm em volta do texto e 1 mm na faixa do título (fundo da cor da moldura).
        let folga = match a.borda & CAIXA_ARREDONDADA != 0 {
            true if a.fundo == a.cor_borda => MM,
            true => 2.0 * MM,
            false => FOLGA_DO_FUNDO,
        };
        acolchoar(&mut linhas, folga);
    }
    // Num item (o texto depois do marcador) a moldura de uma caixa fica no recuo do bloco, não no do texto.
    if a.borda != 0 && b.pagina > 0.0 {
        let esq = a.recuo_esq.max(0.0);
        decorar(&mut linhas, esq, (b.pagina - esq - a.recuo_dir.max(0.0)).max(1.0), a);
    } else {
        decorar(&mut linhas, esq, util, a);
    }
    prender_notas(&mut linhas, trechos, b.largura_da_pagina());
    linhas
}

/// A fórmula em linha mais alta (ou mais funda) que a linha a faz crescer, como o `\lineskip` do
/// TeX: a linha de base desce o que falta em cima e a altura cresce o que falta embaixo.
fn acomodar_formulas(altura: &mut f32, base: &mut f32, pedacos: &mut [Pedaco]) {
    const FOLGA: f32 = 1.0;
    let (mut sobe, mut desce) = (0.0f32, 0.0f32);
    for p in pedacos.iter().filter(|p| p.estilo & MATEMATICA != 0) {
        let f = feather_math::compor_guardado(&p.texto, feather_math::Modo::Linha, p.tamanho);
        sobe = sobe.max(f.altura + p.desloc + FOLGA);
        desce = desce.max(f.profundidade - p.desloc + FOLGA);
    }
    let extra_cima = (sobe - *base).max(0.0);
    if extra_cima > 0.0 {
        *base += extra_cima;
        *altura += extra_cima;
        for p in pedacos.iter_mut() {
            p.y += extra_cima;
        }
    }
    let extra_baixo = (desce - (*altura - *base)).max(0.0);
    *altura += extra_baixo;
}

/// O corpo do texto das notas de rodapé e de fim.
pub const CORPO_NOTA: f32 = 9.0;

/// Compõe o texto de cada nota do parágrafo e o prende à linha onde está a chamada dela.
fn prender_notas(linhas: &mut [Linha], trechos: &[Trecho], largura_pagina: f32) {
    let mut pos = 0;
    for t in trechos {
        let k = linhas.iter().rposition(|l| l.de <= pos).unwrap_or(0);
        match (t.marca.as_deref(), linhas.get_mut(k)) {
            (Some(Objeto::Nota { fim, texto }), Some(l)) => l.notas.push(NotaComposta { fim: *fim, linhas: compor_nota(&t.rotulo, texto, largura_pagina), desl: pos, coluna: None }),
            (Some(Objeto::Comentario { id, fim: false }), Some(l)) => l.comentarios.push(*id),
            _ => {}
        }
        pos += t.texto.chars().count();
    }
}

/// O texto de uma nota: corpo menor, o número sobrescrito na frente (fora do texto).
pub fn compor_nota(numero: &str, texto: &[Trecho], largura_pagina: f32) -> Vec<Linha> {
    // Uma nota dentro de nota não existe: o texto dela não traz objetos.
    let limpo: Vec<Trecho> = texto.iter().filter(|t| t.marca.is_none()).cloned().collect();
    let b = Base::novo(CORPO_NOTA, 0, COR_TEXTO, 0.0, largura_pagina);
    let tam_num = CORPO_NOTA * 0.65;
    let recuo = largura(numero, 0, tam_num) + 3.0;
    let mut linhas = texto_formatado(&limpo, &b, &Atrib { entrelinha: 1.0, ..Atrib::default() });
    for l in &mut linhas {
        for p in &mut l.pedacos {
            p.x += recuo;
        }
        l.x0 += recuo;
    }
    if let Some(l) = linhas.first_mut() {
        let y = l.base;
        l.pedacos.insert(0, Pedaco { x: 0.0, y, texto: numero.to_string(), estilo: 0, tamanho: tam_num, cor: COR_TEXTO, desloc: CORPO_NOTA * 0.35, ..Pedaco::default() });
    }
    linhas
}

/// A folga, em cima e embaixo, do parágrafo com fundo (o `\fboxsep` do LaTeX é 3 pt).
const FOLGA_DO_FUNDO: f32 = 4.0;

/// A linha de um parágrafo sem texto: o cursor fica onde o alinhamento manda.
fn linha_vazia(b: &Base, a: &Atrib, inicio: f32, disponivel: f32) -> Linha {
    let altura = altura_fixa(a, altura_da_linha(b.tamanho * ENTRELINHA * fator_entrelinha(a)));
    let x0 = match a.alinhamento {
        Alinhamento::Centro => inicio + disponivel / 2.0,
        Alinhamento::Direita => inicio + disponivel,
        _ => inicio,
    };
    Linha { altura, x0, base: linha_de_base(altura, b.tamanho), tam: b.tamanho, ..Linha::default() }
}

/// Um milímetro em pontos.
const MM: f32 = 72.0 / 25.4;

/// Bit de `Atrib::borda`: a caixa do `tcolorbox` (fio de 0,5 mm, cantos de 1 mm, texto a 4 mm do fio).
pub const CAIXA_ARREDONDADA: u8 = 16;

/// Fundo e bordas do parágrafo: um retângulo por linha e traços nas bordas pedidas.
fn decorar(linhas: &mut [Linha], esq: f32, util: f32, a: &Atrib) {
    if a.fundo == 0 && a.borda == 0 {
        return;
    }
    let caixa = a.borda & CAIXA_ARREDONDADA != 0;
    let folga = if caixa { 4.0 * MM } else { 4.0 };
    let (x1, x2) = (esq - folga, esq + util + folga);
    // O raio dos cantos: só onde a moldura fecha (em cima no primeiro bloco, embaixo no último).
    let r = if caixa { MM } else { 0.0 };
    let n = linhas.len();
    for (k, l) in linhas.iter_mut().enumerate() {
        let h = l.altura;
        let (topo, pe) = (k == 0 && a.borda & 4 != 0, k + 1 == n && a.borda & 8 != 0);
        let (ya, yb) = (if topo { r } else { 0.0 }, if pe { h - r } else { h });
        let cantos = [(topo, x1 + r, r, -1.0, -1.0), (topo, x2 - r, r, 1.0, -1.0), (pe, x1 + r, h - r, -1.0, 1.0), (pe, x2 - r, h - r, 1.0, 1.0)];
        // shortcut: o canto é um quarto de círculo em 8 traços (o quadro não tem arco); trocar se o raio crescer.
        let arco = |deco: &mut Vec<Deco>, raio: f32, espessura: f32, cor: u32| {
            for (sim, cx, cy, sx, sy) in cantos {
                if !sim || r == 0.0 {
                    continue;
                }
                let ponto = |i: u8| {
                    let t = f32::from(i) * std::f32::consts::FRAC_PI_2 / 8.0;
                    (cx + sx * raio * t.cos(), cy + sy * raio * t.sin())
                };
                for i in 0..8 {
                    let ((xa, ya), (xb, yb)) = (ponto(i), ponto(i + 1));
                    // Cada traço passa um tico da ponta: sem isso a emenda abre um dente do lado de fora da curva.
                    let (dx, dy) = (xb - xa, yb - ya);
                    let k = espessura * 0.12 / dx.hypot(dy).max(0.001);
                    deco.push(Deco::Traco { x1: xa - dx * k, y1: ya - dy * k, x2: xb + dx * k, y2: yb + dy * k, espessura, cor });
                }
            }
        };
        let mut deco = Vec::new();
        if a.fundo != 0 {
            // Um tico a mais para baixo: dois blocos de fundo seguidos (a faixa do cabeçalho) não mostram a emenda.
            deco.push(Deco::Retangulo { x: x1, y: ya, w: x2 - x1, h: yb - ya + if pe { 0.0 } else { 0.75 }, cor: a.fundo });
            // Os cantos arredondados: o fundo entre eles, sem as quinas.
            if topo && r > 0.0 {
                deco.push(Deco::Retangulo { x: x1 + r, y: 0.0, w: x2 - x1 - 2.0 * r, h: r, cor: a.fundo });
            }
            if pe && r > 0.0 {
                deco.push(Deco::Retangulo { x: x1 + r, y: h - r, w: x2 - x1 - 2.0 * r, h: r, cor: a.fundo });
            }
            // O quarto de círculo do canto: um traço da grossura do raio no meio dele.
            arco(&mut deco, r / 2.0, r * 1.1, a.fundo);
        }
        // A chamada (só a barra da esquerda, com cor própria) é mais grossa; a moldura colorida, não.
        let espessura = if caixa { 0.5 * MM } else if a.cor_borda != 0 && a.borda == 1 { 3.0 } else { 0.75 };
        let cor = if a.cor_borda != 0 { a.cor_borda } else { COR_TEXTO };
        let traco = |xa, ya, xb, yb| Deco::Traco { x1: xa, y1: ya, x2: xb, y2: yb, espessura, cor };
        if a.borda & 1 != 0 {
            deco.push(traco(x1, ya, x1, yb));
        }
        if a.borda & 2 != 0 {
            deco.push(traco(x2, ya, x2, yb));
        }
        if topo {
            deco.push(traco(x1 + r, 0.0, x2 - r, 0.0));
        }
        if pe {
            deco.push(traco(x1 + r, h, x2 - r, h));
        }
        arco(&mut deco, r, espessura, cor);
        l.deco.splice(0..0, deco);
    }
}

fn bloco_de_texto(linhas: Vec<Linha>, antes: f32, depois: f32) -> LayoutBloco {
    LayoutBloco { linhas, antes, depois, minimo_antes: 2, minimo_depois: 2, ..LayoutBloco::default() }
}

/// A letra do bloco: a do tipo, ou a do estilo com nome quando o bloco tem um.
fn base_do_estilo(tamanho: f32, extra: u8, limite: f32, e: Option<&Estilo>) -> Base {
    match e {
        Some(e) => Base { efeitos: e.mais_letra.efeitos, ..Base::novo(if e.tamanho > 0.0 { e.tamanho } else { tamanho }, e.bits, if e.cor != 0 { e.cor } else { COR_TEXTO }, 0.0, limite) },
        None => Base::novo(tamanho, extra, COR_TEXTO, 0.0, limite),
    }
}

fn titulo(nivel: u8, texto: &[Trecho], limite: f32, a: &Atrib, en: &Entorno) -> LayoutBloco {
    let i = usize::from(nivel.clamp(1, 6)) - 1;
    let b = base_do_estilo(TAMANHOS_TITULO[i], NEGRITO, limite, en.estilo);
    let antes = en.estilo.and_then(|e| e.antes).unwrap_or(if i == 0 { 18.0 } else { 12.0 });
    let depois = en.estilo.and_then(|e| e.depois).unwrap_or(6.0);
    let (linhas, numero) = com_numero(texto, &b, a, en.numero);
    LayoutBloco { manter_com_proximo: true, numero, ..bloco_de_texto(linhas, antes, depois) }
}

/// O texto com o número automático na frente (título numerado, legenda): o número fica fora do
/// texto, com um vão de meio quadratim, e a primeira linha recua o tanto dele.
fn com_numero(texto: &[Trecho], b: &Base, a: &Atrib, numero: Option<&str>) -> (Vec<Linha>, Option<NumeroDoTitulo>) {
    let Some(n) = numero else { return (texto_formatado(texto, b, a), None) };
    // "Tabela 1:" (LaTeX) leva um espaço normal depois; "Tabela 1 –", meio quadratim.
    let vao = if n.ends_with([':', '.']) { largura(" ", b.extra, b.tamanho) } else { b.tamanho * 0.6 };
    let recuo = largura(n, b.extra, b.tamanho) + vao;
    let linhas = texto_formatado(texto, b, &Atrib { primeira: a.primeira + recuo, ..*a });
    let x = linhas.first().map_or(0.0, |l| l.x0) - recuo;
    (linhas, Some(NumeroDoTitulo { texto: n.to_string(), x, tamanho: b.tamanho, estilo: b.extra, cor: b.cor }))
}

/// Um parágrafo; a legenda (figura, tabela) leva o número e fica junto do que vem depois dela.
fn paragrafo(texto: &[Trecho], limite: f32, a: &Atrib, en: &Entorno) -> LayoutBloco {
    // A legenda em negrito ("\captionsetup{font=bf}") leva o número em negrito também.
    let extra = if a.legenda != 0 && texto.first().is_some_and(|t| t.estilo & NEGRITO != 0) { NEGRITO } else { 0 };
    // A legenda do LaTeX ("Tabela 1:") tem o número na fonte do texto dela (serifada).
    let extra = extra | if en.numero.is_some_and(|n| n.ends_with([':', '.'])) { texto.first().map_or(0, |t| t.estilo & SERIFA) } else { 0 };
    let (linhas, numero) = com_numero(texto, &base_do_estilo(CORPO, extra, limite, en.estilo), a, en.numero);
    LayoutBloco { numero, manter_com_proximo: a.legenda == 2, ..bloco_de_texto(linhas, 0.0, 6.0) }
}

fn item(nivel: u8, texto: &[Trecho], limite: f32, a: &Atrib) -> LayoutBloco {
    let recuo = 18.0 + 18.0 * f32::from(nivel);
    let extra = if a.lista == Lista::Tarefa && a.marcado { COR_CINZA } else { COR_TEXTO };
    let mut linhas = texto_formatado(texto, &Base { pagina: limite, ..Base::novo(CORPO, 0, extra, recuo, limite - recuo) }, a);
    let x = recuo + a.recuo_esq.max(0.0) - 14.0;
    if a.lista == Lista::Tarefa {
        if let Some(l) = linhas.first_mut() {
            caixa_de_tarefa(l, x - 2.0, a.marcado);
        }
        return bloco_de_texto(linhas, 0.0, 3.0);
    }
    LayoutBloco { marcador_x: Some(x), cor_marcador: a.cor_marcador, ..bloco_de_texto(linhas, 0.0, 3.0) }
}

/// O item com rótulo de texto do LaTeX (`label=\textbf{Art. \arabic*º}` do `enumitem`, `align=left`): o
/// rótulo no recuo de primeira linha (negativo) e o texto no recuo do bloco; o rótulo mais largo que o
/// recuo empurra só a primeira linha, com um vão de meio quadratim.
pub fn item_com_rotulo(nivel: u8, texto: &[Trecho], limite: f32, a: &Atrib, rotulo: &str, estilo_rotulo: u8, a_direita: bool) -> LayoutBloco {
    let texto_x = 18.0 + 18.0 * f32::from(nivel) + a.recuo_esq.max(0.0);
    let familia = texto.first().map_or(0, |t| t.estilo & SERIFA);
    let w = if rotulo == BOLINHA { 0.8 * CORPO } else { largura(rotulo, familia | estilo_rotulo, CORPO) };
    // O `align=right` (o padrão do `enumitem`): o rótulo termina meio quadratim antes do texto.
    let x = if a_direita { texto_x - 0.5 * CORPO - w } else { texto_x + a.primeira.min(0.0) }.max(0.0);
    let primeira = (x + w + 0.5 * CORPO - texto_x).max(0.0);
    let linhas = texto_formatado(texto, &Base { pagina: limite, ..Base::novo(CORPO, 0, COR_TEXTO, texto_x, limite - texto_x) }, &Atrib { recuo_esq: 0.0, primeira, ..a.clone() });
    LayoutBloco { marcador_x: Some(x), cor_marcador: a.cor_marcador, estilo_marcador: estilo_rotulo, ..bloco_de_texto(linhas, a.antes.unwrap_or(0.0), a.depois.unwrap_or(3.0)) }
}

/// O rótulo que é a bolinha vazia do `\bigcirc` (as caixas de marcar da classe `exam`): o quadro a
/// desenha, porque a fonte do texto não tem o ◯ grande.
pub const BOLINHA: &str = "\u{25EF}";

/// O espaço `g` em cima da primeira linha, por dentro do fundo e da moldura dos lados (o `\topsep`
/// de um `center` dentro de uma caixa, que não pode ser vão entre blocos sem abrir a moldura).
pub fn acolchoar_em_cima(lay: &mut LayoutBloco, g: f32) {
    let Some(l) = lay.linhas.first_mut() else { return };
    l.altura += g;
    l.base += g;
    l.pedacos.iter_mut().for_each(|p| p.y += g);
    for d in &mut l.deco {
        match d {
            Deco::Retangulo { y, h, .. } if *y <= 0.0 => *h += g,
            Deco::Traco { x1, x2, y1, y2, .. } if x1 == x2 && *y1 <= 0.0 => *y2 += g,
            Deco::Retangulo { y, .. } | Deco::Imagem { y, .. } | Deco::Texto { y, .. } => *y += g,
            Deco::Traco { y1, y2, .. } => {
                *y1 += g;
                *y2 += g;
            }
        }
    }
}

/// A caixa de seleção de um item de tarefa (e o visto, se marcada), na primeira linha.
fn caixa_de_tarefa(l: &mut Linha, x: f32, marcado: bool) {
    let lado = 9.0;
    let y = (l.base - lado * 0.95).max(0.0);
    if marcado {
        l.deco.push(Deco::Retangulo { x, y, w: lado, h: lado, cor: COR_FUNDO_CODIGO });
    }
    let borda = |x1, y1, x2, y2| Deco::Traco { x1, y1, x2, y2, espessura: 0.9, cor: COR_CINZA };
    l.deco.push(borda(x, y, x + lado, y));
    l.deco.push(borda(x + lado, y, x + lado, y + lado));
    l.deco.push(borda(x + lado, y + lado, x, y + lado));
    l.deco.push(borda(x, y + lado, x, y));
    if marcado {
        l.deco.push(Deco::Traco { x1: x + 1.8, y1: y + lado * 0.55, x2: x + lado * 0.42, y2: y + lado - 2.0, espessura: 1.4, cor: COR_TEXTO });
        l.deco.push(Deco::Traco { x1: x + lado * 0.42, y1: y + lado - 2.0, x2: x + lado - 1.6, y2: y + 2.0, espessura: 1.4, cor: COR_TEXTO });
    }
}

fn citacao(texto: &[Trecho], limite: f32, a: &Atrib) -> LayoutBloco {
    let mut linhas = texto_formatado(texto, &Base { pagina: limite, ..Base::novo(CORPO, ITALICO, COR_CITACAO, 22.0, limite - 22.0) }, a);
    for l in &mut linhas {
        l.deco.push(Deco::Traco { x1: 6.0, y1: 0.0, x2: 6.0, y2: l.altura, espessura: 2.5, cor: COR_BORDA });
    }
    bloco_de_texto(linhas, 2.0, 8.0)
}

/// Código: caractere monoespaçado, quebra por colunas, fundo cinza em cada linha.
fn codigo(fonte: &str, limite: f32) -> LayoutBloco {
    let tamanho = 9.5;
    let altura = 13.0;
    let base = linha_de_base(altura, tamanho);
    let colunas = (((limite - 12.0) / (tamanho * 0.6)).floor() as usize).max(1);
    let mut linhas = Vec::new();
    let mut de = 0;
    for crua in fonte.split('\n') {
        let cs: Vec<char> = crua.chars().collect();
        let partes: Vec<String> = cs.chunks(colunas).map(|p| p.iter().collect::<String>()).chain(cs.is_empty().then(String::new)).collect();
        for parte in partes {
            let n = parte.chars().count();
            let pedacos = vec![Pedaco { x: 6.0, y: base, texto: parte, estilo: CODIGO, tamanho, cor: COR_TEXTO, de, ..Pedaco::default() }];
            let deco = vec![Deco::Retangulo { x: 0.0, y: 0.0, w: limite, h: altura, cor: COR_FUNDO_CODIGO }];
            linhas.push(Linha { altura, pedacos, deco, de, x0: 6.0, base, tam: tamanho, ..Linha::default() });
            de += n;
        }
        de += 1; // a quebra de linha do código
    }
    acolchoar(&mut linhas, 4.0);
    LayoutBloco { linhas, antes: 4.0, depois: 10.0, minimo_antes: 2, minimo_depois: 2, ..LayoutBloco::default() }
}

/// A listagem com o estilo do `listings` (`Bloco.latex`): pares `chave=valor` separados por espaço,
/// cores em `RRGGBBAA` hexadecimal. `tam` corpo; `texto`, `chave`, `comentario`, `aspas` cores do
/// realce; `negrito`/`italico` na palavra-chave/no comentário; `fundo`; `moldura` cor do fio.
pub fn codigo_latex(linguagem: &str, fonte: &str, estilo: &str, limite: f32, a: &Atrib) -> LayoutBloco {
    let mut tamanho = 9.5;
    let (mut cores, mut fundo, mut moldura, mut chave, mut comentario) = ([COR_TEXTO, 0, 0, 0], 0, 0, NEGRITO, 0);
    for par in estilo.split_whitespace() {
        let (k, v) = par.split_once('=').unwrap_or((par, ""));
        let cor = u32::from_str_radix(v, 16).ok();
        match (k, cor) {
            ("tam", _) => tamanho = v.parse().unwrap_or(tamanho),
            ("texto", Some(c)) => cores[0] = c,
            ("chave", Some(c)) => cores[1] = c,
            ("comentario", Some(c)) => cores[2] = c,
            ("aspas", Some(c)) => cores[3] = c,
            ("fundo", Some(c)) => fundo = c,
            ("moldura", Some(c)) => moldura = c,
            ("sem_negrito", _) => chave = 0,
            ("italico", _) => comentario = ITALICO,
            _ => {}
        }
    }
    // Os que não ganharam cor própria ficam na do texto.
    for i in 1..4 {
        if cores[i] == 0 {
            cores[i] = cores[0];
        }
    }
    // O `frame=single` do `listings`: `framesep` de 3pt entre o fio e o texto.
    let (x0, largura) = (a.recuo_esq, (limite - a.recuo_esq - a.recuo_dir).max(20.0));
    let folga = if moldura != 0 { 3.4 } else { 6.0 };
    let altura = tamanho * 1.22;
    let base = linha_de_base(altura, tamanho);
    let passo = tamanho * largura_do_caractere('m', CODIGO) as f32 / 1000.0;
    let colunas = (((largura - 2.0 * folga) / passo).floor() as usize).max(1);
    let mut linhas = Vec::new();
    let mut de = 0;
    for (crua, trechos) in fonte.split('\n').zip(crate::realce::realcar(linguagem, fonte)) {
        let cs: Vec<char> = crua.chars().collect();
        let mut inicio = 0;
        loop {
            let fim = (inicio + colunas).min(cs.len());
            let pedacos = trechos.iter().filter(|(r, _)| r.start < fim && r.end > inicio).map(|(r, t)| {
                let (a, b) = (r.start.max(inicio), r.end.min(fim));
                let (estilo, cor) = match t {
                    crate::realce::Tipo::Texto => (CODIGO, cores[0]),
                    crate::realce::Tipo::Chave => (CODIGO | chave, cores[1]),
                    crate::realce::Tipo::Comentario => (CODIGO | comentario, cores[2]),
                    crate::realce::Tipo::Aspas => (CODIGO, cores[3]),
                };
                Pedaco { x: x0 + folga + (a - inicio) as f32 * passo, y: base, texto: cs[a..b].iter().collect(), estilo, tamanho, cor, de: de + a, ..Pedaco::default() }
            }).collect();
            linhas.push(Linha { altura, pedacos, de: de + inicio, x0: x0 + folga, base, tam: tamanho, ..Linha::default() });
            inicio = fim;
            if inicio >= cs.len() {
                break;
            }
        }
        de += cs.len() + 1;
    }
    acolchoar(&mut linhas, folga);
    // shortcut: o `frameround` sai com cantos retos; desenhar o arco quando houver Deco de curva.
    let n = linhas.len();
    for (i, l) in linhas.iter_mut().enumerate() {
        let (x1, h) = (x0 + largura, l.altura);
        if fundo != 0 {
            l.deco.push(Deco::Retangulo { x: x0, y: 0.0, w: largura, h, cor: fundo });
        }
        if moldura != 0 {
            let fio = |x1, y1, x2, y2| Deco::Traco { x1, y1, x2, y2, espessura: 0.4, cor: moldura };
            l.deco.extend([fio(x0, 0.0, x0, h), fio(x1, 0.0, x1, h)]);
            if i == 0 {
                l.deco.push(fio(x0, 0.0, x1, 0.0));
            }
            if i + 1 == n {
                l.deco.push(fio(x0, h, x1, h));
            }
        }
    }
    LayoutBloco { linhas, antes: 6.0, depois: 6.0, minimo_antes: 2, minimo_depois: 2, ..LayoutBloco::default() }
}

/// Aumenta a primeira e a última linha (o "padding" do bloco de código).
fn acolchoar(linhas: &mut [Linha], extra: f32) {
    let n = linhas.len();
    for (i, l) in linhas.iter_mut().enumerate() {
        let em_cima = if i == 0 { extra } else { 0.0 };
        let em_baixo = if i + 1 == n { extra } else { 0.0 };
        l.altura += em_cima + em_baixo;
        for p in &mut l.pedacos {
            p.y += em_cima;
        }
        l.base += em_cima;
        for d in &mut l.deco {
            if let Deco::Retangulo { h, .. } = d {
                *h += em_cima + em_baixo;
            }
        }
    }
}

fn celulas_da_linha(linha: &[Vec<Trecho>], colunas: usize, largura_col: f32, cabecalho: bool, mesclagens: &[MesclagemTabela], indice_linha: usize, estica: f32, fundo: u32) -> Linha {
    let tam = 10.0;
    let extra = if cabecalho { NEGRITO } else { 0 };
    let pagina = largura_col * colunas as f32;
    let compostas: Vec<Vec<Linha>> = (0..colunas).map(|c| {
        let regiao = mesclagens.iter().find(|m| m.contem(indice_linha, c));
        if regiao.is_some_and(|m| m.coluna != c) {
            return Vec::new(); // interior da mesclagem: sem texto ou borda interna
        }
        let largura = regiao.map_or(largura_col, |m| largura_col * m.colunas as f32);
        texto_celula(linha.get(c).map_or(&[][..], Vec::as_slice), tam, extra, largura - 10.0, pagina, false)
    }).collect();
    let linhas_max = compostas.iter().map(Vec::len).max().unwrap_or(1);
    let altura = altura_da_linha(tam * ENTRELINHA * estica);
    let total = altura * linhas_max as f32 + 8.0;
    let mut saida = Linha { altura: total, ..Linha::default() };
    let base = 4.0 + linha_de_base(altura, tam);
    if cabecalho || fundo != 0 {
        let cor = if cabecalho { COR_FUNDO_CODIGO } else { fundo };
        saida.deco.push(Deco::Retangulo { x: 0.0, y: 0.0, w: largura_col * colunas as f32, h: total, cor });
    }
    for (c, celula) in compostas.into_iter().enumerate() {
        let x0 = largura_col * c as f32;
        saida.celulas.push(InfoCelula { x0: x0 + 5.0, base, tam });
        for (k, l) in celula.into_iter().enumerate() {
            saida.notas.extend(l.notas.into_iter().map(|n| NotaComposta { coluna: Some(c as u16), ..n }));
            saida.comentarios.extend(l.comentarios);
            for mut p in l.pedacos {
                p.x += x0 + 5.0;
                p.y += 4.0 + altura * k as f32;
                p.celula = Some(c as u16);
                saida.pedacos.push(p);
            }
        }
        if !mesclagens.iter().any(|m| m.contem(indice_linha, c) && m.coluna != c) {
            saida.deco.push(Deco::Traco { x1: x0, y1: 0.0, x2: x0, y2: total, espessura: 0.5, cor: COR_BORDA });
        }
    }
    let fim = largura_col * colunas as f32;
    saida.deco.push(Deco::Traco { x1: fim, y1: 0.0, x2: fim, y2: total, espessura: 0.5, cor: COR_BORDA });
    saida.deco.push(Deco::Traco { x1: 0.0, y1: total, x2: fim, y2: total, espessura: 0.5, cor: COR_BORDA });
    saida
}

/// Tabela: colunas iguais, uma `Linha` por linha da tabela (a paginação nunca parte uma linha).
/// A entrelinha do bloco estica as linhas (o `\arraystretch` do LaTeX). O fundo pinta uma linha sim,
/// outra não, da segunda em diante (o `\rowcolors`): as ímpares, ou as pares se `marcado`.
fn tabela(cabecalho: bool, linhas: &[Vec<Celula>], limite: f32, mesclagens: &[MesclagemTabela], a: &Atrib) -> LayoutBloco {
    let estica = fator_entrelinha(a);
    let colunas = linhas.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let largura_col = limite / colunas as f32;
    let fundo = |i: usize| if i >= 1 && ((i + 1) % 2 == 0) == a.marcado { a.fundo } else { 0 };
    let mut compostas: Vec<Linha> = linhas.iter().enumerate().map(|(i, l)| celulas_da_linha(l, colunas, largura_col, cabecalho && i == 0, mesclagens, i, estica, fundo(i))).collect();
    if let Some(primeira) = compostas.first_mut() {
        let fim = largura_col * colunas as f32;
        primeira.deco.push(Deco::Traco { x1: 0.0, y1: 0.0, x2: fim, y2: 0.0, espessura: 0.5, cor: COR_BORDA });
    }
    com_moldura(LayoutBloco { linhas: compostas, antes: 4.0, depois: 10.0, minimo_antes: 1, minimo_depois: 1, ..LayoutBloco::default() }, limite, a)
}

/// Uma medida do LaTeX em pontos (`3cm`, `25mm`, `2in`, `40pt`); `None` se depende da página.
fn medida_latex(v: &str) -> Option<f32> {
    let v = v.trim();
    let fim = v.find(|c: char| c.is_ascii_alphabetic())?;
    let n: f32 = v[..fim].trim().replace(',', ".").parse().ok()?;
    let por = match &v[fim..] {
        "pt" => 1.0,
        "cm" => 28.3465,
        "mm" => 2.83465,
        "in" => 72.0,
        "em" => CORPO,
        _ => return None,
    };
    Some(n * por)
}

/// As colunas de um `tabular`: o alinhamento de cada uma (`l`, `c`, `r`; `p` quebra o texto na largura
/// dela), os `|` em cada divisa (uma a mais que as colunas) e se o `@{}` tira o `\tabcolsep` da ponta
/// esquerda e da direita.
fn colunas_latex(spec: &str) -> (Vec<char>, Vec<Option<f32>>, Vec<u8>, bool, bool) {
    let (mut alin, mut largs, mut barras, mut sem_esq, mut sem_dir) = (Vec::new(), Vec::new(), vec![0u8], false, false);
    let cs: Vec<char> = spec.chars().collect();
    let mut i = 0;
    // O `>{\RaggedLeft}`/`>{\centering}` antes de uma coluna `p`: o alinhamento dela.
    let mut proximo = 'l';
    // O `{…}` que vem depois de `p`, `m`, `b`, `@`, `>`, `<` e `*{n}`.
    let pular_grupo = |i: &mut usize| {
        if cs.get(*i) == Some(&'{') {
            let mut nivel = 0;
            while let Some(c) = cs.get(*i) {
                nivel += match c { '{' => 1, '}' => -1, _ => 0 };
                *i += 1;
                if nivel == 0 {
                    break;
                }
            }
        }
    };
    while let Some(&c) = cs.get(i) {
        i += 1;
        match c {
            'l' | 'c' | 'r' | 'X' | 'S' => {
                alin.push(if c == 'X' { 'l' } else if c == 'S' { 'c' } else { c });
                largs.push(None);
                barras.push(0);
            }
            'p' | 'm' | 'b' => {
                let ini = i;
                pular_grupo(&mut i);
                let medida: String = cs[(ini + 1).min(i)..i.saturating_sub(1).max(ini)].iter().collect();
                alin.push(std::mem::replace(&mut proximo, 'l'));
                largs.push(medida_latex(&medida));
                barras.push(0);
            }
            '|' => *barras.last_mut().unwrap_or(&mut 0) += 1,
            '@' => {
                if alin.is_empty() { sem_esq = true } else { sem_dir = true }
                pular_grupo(&mut i);
            }
            '>' | '<' | '!' | '*' => {
                let ini = i;
                pular_grupo(&mut i);
                if c == '>' {
                    let g: String = cs[ini..i].iter().collect();
                    proximo = if g.contains("RaggedLeft") || g.contains("raggedleft") { 'r' } else if g.contains("entering") { 'c' } else { 'l' };
                }
                if c == '*' {
                    pular_grupo(&mut i);
                }
            }
            _ => {}
        }
    }
    // O `@` do meio (entre colunas) não é ponta.
    if sem_dir && !spec.trim_end().ends_with('}') {
        sem_dir = false;
    }
    (alin, largs, barras, sem_esq, sem_dir)
}

/// Tabela de um `tabular` do LaTeX (11 pt), com os fios e as colunas do código: cada coluna da largura
/// do maior texto, mais o `\tabcolsep` (6 pt) de cada lado; a linha com a altura do `\strut`
/// (13,6 pt × `\arraystretch`); o `\toprule`/`\bottomrule` (0,08 em) e o `\midrule` (0,05 em) com o
/// espaço do `booktabs`; o `\hline` e o `\cline` de 0,4 pt sem espaço; os `|` do código. A tabela
/// tem a largura dela (não a da página), no meio se o `\centering` pediu.
/// shortcut: a coluna `p{…}` com medida da página (`0.3\linewidth`) e o `\multirow` saem como `l` sem
/// quebrar o texto; a célula mesclada fica no meio. Trocar quando aparecer documento que precise.
pub fn tabela_latex(linhas: &[Vec<Celula>], regua: &str, limite: f32, mesclagens: &[MesclagemTabela], a: &Atrib) -> LayoutBloco {
    let (spec, fios) = regua.split_once('\n').unwrap_or((regua, ""));
    let fios: Vec<&str> = fios.split(';').collect();
    let (alin, largs, barras, sem_esq, sem_dir) = colunas_latex(spec);
    let n = linhas.iter().map(Vec::len).max().unwrap_or(0).max(alin.len()).max(1);
    let tam = CORPO;
    let (em, ex, sep, fio) = (tam, 0.43 * tam, 6.0, 0.4);
    let estica = fator_entrelinha(a);
    let altura = 13.6 * estica;
    let mesclada = |i: usize, c: usize| mesclagens.iter().find(|m| m.contem(i, c));
    // Cada célula composta numa linha só, e a largura do que ela mostra.
    let compostas: Vec<Vec<(Vec<Linha>, f32)>> = linhas.iter().map(|l| (0..n).map(|c| {
        let ls = texto_celula(l.get(c).map_or(&[][..], Vec::as_slice), tam, 0, largs.get(c).copied().flatten().unwrap_or(1.0e6), limite, true);
        let imagens = ls.iter().flat_map(|l| &l.deco).filter_map(|d| match d { Deco::Imagem { x, w, .. } => Some(x + w), _ => None });
        let w = ls.iter().flat_map(|l| &l.pedacos).map(|p| p.x + largura(&p.texto, p.estilo, p.tamanho)).chain(imagens).fold(0.0, f32::max);
        (ls, w)
    }).collect()).collect();
    let mut larg: Vec<f32> = (0..n).map(|c| largs.get(c).copied().flatten().unwrap_or(0.0)).collect();
    for (i, l) in compostas.iter().enumerate() {
        for (c, (_, w)) in l.iter().enumerate() {
            if mesclada(i, c).is_none() && largs.get(c).copied().flatten().is_none() {
                larg[c] = larg[c].max(*w);
            }
        }
    }
    let pad_esq = |c: usize| if c == 0 && sem_esq { 0.0 } else { sep };
    let pad_dir = |c: usize| if c + 1 == n && sem_dir { 0.0 } else { sep };
    // A célula mesclada mais larga que as colunas dela alarga a última.
    for m in mesclagens {
        let (fim, w) = ((m.coluna + m.colunas).min(n), compostas.get(m.linha).and_then(|l| l.get(m.coluna)).map_or(0.0, |x| x.1));
        let tem: f32 = (m.coluna..fim).map(|c| larg[c] + pad_esq(c) + pad_dir(c)).sum::<f32>() - pad_esq(m.coluna) - pad_dir(fim - 1);
        if w > tem && fim > m.coluna {
            larg[fim - 1] += w - tem;
        }
    }
    let mut xs = vec![0.0f32; n + 1];
    for c in 0..n {
        xs[c + 1] = xs[c] + pad_esq(c) + larg[c] + pad_dir(c);
    }
    let total = xs[n];
    // O recuo da caixa em volta (`tcolorbox`): a tabela começa onde começa o texto dela.
    let x0 = a.recuo_esq + if a.alinhamento == Alinhamento::Centro { ((limite - a.recuo_esq - a.recuo_dir - total) / 2.0).max(0.0) } else { 0.0 };
    let fundo = |i: usize| if i >= 1 && ((i + 1) % 2 == 0) == a.marcado { a.fundo } else { 0 };
    // Os fios de uma divisa, a partir de `y`: devolve o `y` depois deles.
    let divisa = |k: usize, mut y: f32, deco: &mut Vec<Deco>| -> f32 {
        let mut anterior_h = false;
        for f in fios.get(k).copied().unwrap_or("").split(',').filter(|f| !f.is_empty()) {
            let faixa = |f: &str| {
                let (de, ate) = f[1..].split_once('-').unwrap_or((&f[1..], &f[1..]));
                let (de, ate) = (de.trim().parse::<usize>().unwrap_or(1).max(1) - 1, ate.trim().parse::<usize>().unwrap_or(n).min(n));
                (x0 + xs[de.min(n)], x0 + xs[ate.max(de + 1).min(n)])
            };
            let mut traco = |y: f32, e: f32, (x1, x2): (f32, f32)| deco.push(Deco::Traco { x1, y1: y + e / 2.0, x2, y2: y + e / 2.0, espessura: e, cor: COR_TEXTO });
            match &f[..1] {
                "T" => {
                    traco(y, 0.08 * em, (x0, x0 + total));
                    y += 0.08 * em + 0.65 * ex;
                }
                "M" => {
                    y += 0.4 * ex;
                    traco(y, 0.05 * em, (x0, x0 + total));
                    y += 0.05 * em + 0.65 * ex;
                }
                "B" => {
                    y += 0.4 * ex;
                    traco(y, 0.08 * em, (x0, x0 + total));
                    y += 0.08 * em;
                }
                "H" => {
                    if anterior_h {
                        y += 2.0; // \doublerulesep
                    }
                    traco(y, fio, (x0, x0 + total));
                    y += fio;
                }
                "c" => {
                    y += 0.4 * ex;
                    // O `\cmidrule` sem `(lr)` vai de ponta a ponta das colunas.
                    traco(y, 0.03 * em, faixa(f));
                    y += 0.03 * em + 0.65 * ex;
                }
                // O `\cline` se sobrepõe à linha de cima: não ocupa espaço.
                "C" => traco(y - fio, fio, faixa(f)),
                _ => {}
            }
            anterior_h = f == "H";
        }
        y
    };
    let mut saida: Vec<Linha> = Vec::new();
    for (i, l) in compostas.into_iter().enumerate() {
        let mut linha = Linha::default();
        let topo = divisa(i, 0.0, &mut linha.deco);
        // A letra maior que a do `\strut` (`{\Huge 41}`) empurra a linha para baixo: o TeX soma a altura
        // da caixa mais alta da primeira linha (uns 0,72 em acima da base), não só a do `\strut`.
        let maior = l.iter().filter_map(|(ls, _)| ls.first()).flat_map(|l| &l.pedacos).map(|p| p.tamanho).fold(0.0, f32::max);
        let a_mais = (0.72 * maior - 0.7 * altura).max(0.0);
        let base = topo + 0.7 * altura + a_mais;
        // A coluna `p` quebra o texto: a linha da tabela cresce de uma linha de texto por vez.
        let alta = altura * l.iter().map(|(ls, _)| ls.len()).max().unwrap_or(1).max(1) as f32 + a_mais;
        if fundo(i) != 0 {
            linha.deco.insert(0, Deco::Retangulo { x: x0, y: topo, w: total, h: alta, cor: fundo(i) });
        }
        for (c, (ls, w)) in l.into_iter().enumerate() {
            let regiao = mesclada(i, c);
            if regiao.is_some_and(|m| m.coluna != c) {
                linha.celulas.push(InfoCelula { x0: x0 + xs[c] + pad_esq(c), base, tam });
                continue;
            }
            let (ini, fim) = regiao.map_or((c, c + 1), |m| (m.coluna, (m.coluna + m.colunas).min(n)));
            let (esq, dir) = (x0 + xs[ini] + pad_esq(ini), x0 + xs[fim] - pad_dir(fim - 1));
            let x = match (regiao.is_some(), alin.get(c).copied().unwrap_or('l')) {
                (true, _) | (false, 'c') => esq + ((dir - esq) - w) / 2.0,
                (false, 'r') => dir - w,
                _ => esq,
            };
            linha.celulas.push(InfoCelula { x0: x, base, tam });
            for (k, l) in ls.into_iter().enumerate() {
                // A imagem da célula (o logo do `\multirow`): a mais alta que a linha começa no topo dela e
                // desce por cima da de baixo.
                for d in l.deco {
                    if let Deco::Imagem { x: dx, y, w, h, origem } = d {
                        linha.deco.push(Deco::Imagem { x: dx + x, y: (y - l.base + base + altura * k as f32).max(topo), w, h, origem });
                    }
                }
                linha.notas.extend(l.notas.into_iter().map(|nota| NotaComposta { coluna: Some(c as u16), ..nota }));
                linha.comentarios.extend(l.comentarios);
                for mut p in l.pedacos {
                    p.x += x;
                    p.y = base + altura * k as f32;
                    p.celula = Some(c as u16);
                    linha.pedacos.push(p);
                }
            }
        }
        let mut fim = topo + alta;
        // Os `|` atravessam a linha (e os `\hline` dela).
        for (k, b) in barras.iter().enumerate().take(n + 1) {
            // Dentro de uma célula mesclada o `|` não passa (o `\multicolumn` tem as colunas dele).
            if mesclagens.iter().any(|m| m.linha == i && m.coluna < k && k < m.coluna + m.colunas) {
                continue;
            }
            for j in 0..*b {
                let x = x0 + xs[k] + if k == n { -(f32::from(j) * 2.0) } else { f32::from(j) * 2.0 };
                linha.deco.push(Deco::Traco { x1: x, y1: 0.0, x2: x, y2: fim, espessura: fio, cor: COR_TEXTO });
            }
        }
        if i + 1 == linhas.len() {
            fim = divisa(i + 1, fim, &mut linha.deco);
        }
        linha.altura = fim;
        saida.push(linha);
    }
    com_moldura(LayoutBloco { linhas: saida, antes: 4.0, depois: 10.0, minimo_antes: 1, minimo_depois: 1, ..LayoutBloco::default() }, limite, a)
}

/// A tabela dentro de uma caixa (`\fbox`, `tcolorbox`): a moldura em volta, como a dos parágrafos dela,
/// e os espaços da caixa (sem eles as laterais se partem entre os blocos). O `fundo` da tabela é o das
/// linhas alternadas, não o da caixa.
fn com_moldura(mut lay: LayoutBloco, limite: f32, a: &Atrib) -> LayoutBloco {
    if a.borda != 0 {
        // O `\fboxsep` (3 pt) entre a moldura de cima e a primeira linha da tabela.
        if a.borda & 4 != 0 && a.fundo == 0 {
            acolchoar_em_cima(&mut lay, 3.0);
        }
        decorar(&mut lay.linhas, a.recuo_esq, (limite - a.recuo_esq - a.recuo_dir).max(1.0), &Atrib { fundo: 0, ..*a });
        lay.antes = a.antes.unwrap_or(lay.antes);
        lay.depois = a.depois.unwrap_or(lay.depois);
    }
    lay
}

/// Uma linha única que não se parte (imagem, equação, régua).
fn indivisivel(linha: Linha, antes: f32, depois: f32) -> LayoutBloco {
    LayoutBloco { linhas: vec![linha], antes, depois, minimo_antes: 1, minimo_depois: 0, ..LayoutBloco::default() }
}

fn imagem(origem: &str, legenda: &[Trecho], w: f32, h: f32, limite: f32, numero: Option<&str>) -> LayoutBloco {
    let (mut w, mut h) = if w > 0.0 && h > 0.0 { (w, h) } else { (limite * 0.7, limite * 0.5) };
    let escala = (limite / w).min(520.0 / h).min(1.0);
    w *= escala;
    h *= escala;
    let deco = vec![Deco::Imagem { x: (limite - w) / 2.0, y: 0.0, w, h, origem: origem.to_string() }];
    let mut bloco = indivisivel(Linha { altura: h, deco, ..Linha::default() }, 6.0, 4.0);
    if !legenda.is_empty() {
        // A do LaTeX ("Figura 1: …") sai como o `\caption`: o número na frente, na letra e no tamanho do texto.
        let mut texto = match numero {
            Some(n) => {
                let estilo = legenda.first().map_or(0, |t| t.estilo);
                let mut com_numero = vec![Trecho { estilo, ..Trecho::simples(&format!("{n} ")) }];
                com_numero.extend_from_slice(legenda);
                texto_corrido(&com_numero, CORPO, 0, COR_TEXTO, 0.0, limite)
            }
            None => texto_corrido(legenda, 9.5, ITALICO, COR_CITACAO, 0.0, limite),
        };
        // A linha inteira no meio (cada pedaço sozinho no meio encavalaria "(a) " e o resto).
        for l in &mut texto {
            let fim = l.pedacos.iter().map(|p| p.x + largura(&p.texto, p.estilo, p.tamanho)).fold(0.0, f32::max);
            let desloc = ((limite - fim) / 2.0).max(0.0);
            for p in &mut l.pedacos {
                p.x += desloc;
            }
        }
        bloco.linhas.extend(texto);
        bloco.minimo_antes = 2;
    }
    bloco.depois = 10.0;
    bloco
}

/// O espaço acima e abaixo da equação em destaque (o `\abovedisplayskip` de 10 pt da classe `article`).
const ESPACO_DA_EQUACAO: f32 = 10.0;

/// Equação em destaque: a fórmula composta pelo `feather-math` (estilo display), centrada, e o número
/// de cada linha numerada encostado à direita. `numero` = o primeiro número automático dela.
fn equacao(latex: &str, limite: f32, numero: Option<&str>) -> LayoutBloco {
    let tamanho = CORPO;
    let estilo = MATEMATICA | EXIBIDA;
    let f = feather_math::compor_guardado(latex, feather_math::Modo::Exibida, tamanho);
    let folga = 2.0;
    let base = folga + f.altura;
    let altura = (f.altura + f.profundidade + 2.0 * folga).max(tamanho * ENTRELINHA);
    // O número sai na fonte da fórmula, em pé (o `\tag` do LaTeX usa a romana do texto, a mesma Latin Modern).
    let rotulos: Vec<(f32, String)> = rotulos_da_equacao(&f, numero).into_iter().map(|(y, r)| (y, format!("\\text{{{r}}}"))).collect();
    // O número não pode encostar na fórmula: sem espaço, ela vai para a esquerda (como o TeX faz).
    let maior = rotulos.iter().map(|(_, r)| largura(r, MATEMATICA, tamanho)).fold(0.0, f32::max);
    let mut x = ((limite - f.largura) / 2.0).max(0.0);
    if maior > 0.0 && x + f.largura > limite - maior - tamanho {
        x = (limite - maior - tamanho - f.largura).max(0.0);
    }
    let p = Pedaco { x, y: base, texto: latex.to_string(), estilo, tamanho, cor: COR_TEXTO, ..Pedaco::default() };
    let deco = rotulos
        .into_iter()
        .map(|(y, texto)| Deco::Texto { x: limite - largura(&texto, MATEMATICA, tamanho), y: base + y, tamanho, estilo: MATEMATICA, cor: COR_TEXTO, texto })
        .collect();
    let linha = Linha { altura, pedacos: vec![p], deco, x0: x, base, tam: tamanho, ..Linha::default() };
    indivisivel(linha, ESPACO_DA_EQUACAO - folga, ESPACO_DA_EQUACAO - folga)
}

/// "(1)", "(2)"… (ou o `\tag`) de cada linha numerada, com a posição da linha de base dela.
fn rotulos_da_equacao(f: &feather_math::Formula, numero: Option<&str>) -> Vec<(f32, String)> {
    let mut n: u32 = numero.and_then(|s| s.parse().ok()).unwrap_or(1);
    f.linhas
        .iter()
        .filter_map(|l| match &l.numero {
            feather_math::Numero::Automatico => {
                let r = format!("({n})");
                n += 1;
                Some((l.base, r))
            }
            feather_math::Numero::Tag(t) => Some((l.base, format!("({t})"))),
            feather_math::Numero::Nenhum => None,
        })
        .collect()
}

/// A quebra de página aparece como uma linha discreta no fim da página; o que vem depois começa a seguinte.
fn quebra_de_pagina(limite: f32) -> LayoutBloco {
    marca_de_quebra("quebra de página", limite, true)
}

/// Uma quebra (de página, coluna ou seção): o nome no meio de um traço fino; `pagina` = o resto começa a página seguinte.
fn marca_de_quebra(texto: &str, limite: f32, pagina: bool) -> LayoutBloco {
    let tamanho = 8.5;
    let altura = 16.0;
    let w = largura(texto, ITALICO, tamanho);
    let x = ((limite - w) / 2.0).max(0.0);
    let base = linha_de_base(altura, tamanho);
    let p = Pedaco { x, y: base, texto: texto.to_string(), estilo: ITALICO, tamanho, cor: COR_CINZA, ..Pedaco::default() };
    let deco = vec![
        Deco::Traco { x1: 0.0, y1: altura / 2.0, x2: (x - 6.0).max(0.0), y2: altura / 2.0, espessura: 0.5, cor: COR_BORDA },
        Deco::Traco { x1: x + w + 6.0, y1: altura / 2.0, x2: limite, y2: altura / 2.0, espessura: 0.5, cor: COR_BORDA },
    ];
    let linha = Linha { altura, pedacos: vec![p], deco, x0: x, base, tam: tamanho, ..Linha::default() };
    LayoutBloco { quebra_depois: pagina, so_na_tela: true, ..indivisivel(linha, 2.0, 2.0) }
}

/// A quebra de seção: como a de página, menos a contínua; a de página par ou ímpar ainda escolhe a paridade.
fn quebra_de_secao(inicio: InicioDeSecao, limite: f32) -> LayoutBloco {
    let (texto, paridade) = match inicio {
        InicioDeSecao::ProximaPagina => ("quebra de seção (próxima página)", None),
        InicioDeSecao::Continua => ("quebra de seção (contínua)", None),
        InicioDeSecao::PaginaPar => ("quebra de seção (página par)", Some(true)),
        InicioDeSecao::PaginaImpar => ("quebra de seção (página ímpar)", Some(false)),
    };
    LayoutBloco { pagina_par: paridade, ..marca_de_quebra(texto, limite, inicio != InicioDeSecao::Continua) }
}

/// A régua: na cor da borda do bloco (o `{\color{x}\hrule}` do LaTeX), ou cinza.
fn regua(limite: f32, a: &Atrib) -> LayoutBloco {
    let (cor, espessura) = if a.cor_borda != 0 { (a.cor_borda, 1.0) } else { (COR_BORDA, 0.75) };
    let deco = vec![Deco::Traco { x1: 0.0, y1: 5.0, x2: limite, y2: 5.0, espessura, cor }];
    indivisivel(Linha { altura: 10.0, deco, base: 8.0, tam: CORPO, ..Linha::default() }, 4.0, 4.0)
}

/// Uma entrada do sumário: o título e para onde ela aponta.
#[derive(Clone, Debug, PartialEq)]
pub struct EntradaSumario {
    pub id: u64,
    pub nivel: u8,
    pub texto: String,
}

/// O sumário: uma linha por título até `niveis`. A altura não depende do número da
/// página (que sai só no quadro), então a paginação não precisa rodar duas vezes.
pub fn sumario(entradas: &[EntradaSumario], niveis: u8, (numeros, pontilhado, latex): (bool, bool, bool), limite: f32, rotulo: &str, secao: Option<&Estilo>) -> LayoutBloco {
    if latex {
        return sumario_latex(entradas, niveis, numeros, limite, rotulo, secao);
    }
    let marcas = u8::from(numeros) | if numeros && pontilhado { 2 } else { 0 };
    let mut linhas = texto_corrido(&[Trecho::simples(rotulo)], 16.0, NEGRITO, COR_TEXTO, 0.0, limite);
    let altura = 18.0;
    for e in entradas.iter().filter(|e| e.nivel <= niveis) {
        let recuo = 14.0 * f32::from(e.nivel.saturating_sub(1));
        let estilo = if e.nivel == 1 { NEGRITO } else { 0 };
        let texto = cortar(&e.texto, estilo, CORPO, limite - recuo - if numeros { 40.0 } else { 0.0 });
        let p = Pedaco { x: recuo, y: linha_de_base(altura, CORPO), texto, estilo, tamanho: CORPO, cor: COR_TEXTO, ..Pedaco::default() };
        linhas.push(Linha { altura, pedacos: vec![p], alvo: Some(e.id), sumario: marcas, ..Linha::default() });
    }
    if linhas.len() == 1 {
        let p = Pedaco { x: 0.0, y: linha_de_base(altura, CORPO), texto: "—".to_string(), estilo: ITALICO, tamanho: CORPO, cor: COR_CINZA, ..Pedaco::default() };
        linhas.push(Linha { altura, pedacos: vec![p], ..Linha::default() });
    }
    LayoutBloco { linhas, antes: 6.0, depois: 14.0, minimo_antes: 2, minimo_depois: 2, ..LayoutBloco::default() }
}

/// O `\tableofcontents` da classe `article` (11 pt): o título como um `\section*`, a seção em
/// negrito, sem pontilhado e com 1 em antes; a subseção recuada 1,5 em, com o número numa coluna de
/// 2,3 em e o pontilhado. A lista de figuras e a de tabelas são como a subseção.
fn sumario_latex(entradas: &[EntradaSumario], niveis: u8, numeros: bool, limite: f32, rotulo: &str, secao: Option<&Estilo>) -> LayoutBloco {
    let em = CORPO;
    // O espaço de 2,3 ex depois do título.
    let mut linhas = titulo_de_lista(rotulo, secao, (14.4, NEGRITO | SERIFA, 2.3 * 0.43 * em), limite);
    let altura = 1.2 * em + 0.2;
    for e in entradas.iter().filter(|e| e.nivel <= niveis) {
        let (recuo, coluna) = match e.nivel {
            1 => (0.0, 1.5 * em),
            3 => (3.8 * em, 3.2 * em),
            n if n >= 4 => (7.0 * em, 4.1 * em),
            _ => (1.5 * em, 2.3 * em),
        };
        let secao = e.nivel == 1;
        let estilo = SERIFA | if secao { NEGRITO } else { 0 };
        // "1.2  Título" (sumário) ou "Figura 1 – Título" (listas): o número e o título separados.
        let (numero, titulo) = match e.texto.split_once("  ") {
            Some((n, t)) => (n.to_string(), t.to_string()),
            None => match e.texto.split_once(" – ") {
                Some((n, t)) => (n.rsplit(' ').next().unwrap_or("").to_string(), t.to_string()),
                None => (String::new(), e.texto.clone()),
            },
        };
        let antes = if secao { em } else { 0.0 };
        let y = antes + linha_de_base(altura, CORPO);
        let mut pedacos = Vec::new();
        if !numero.is_empty() {
            pedacos.push(Pedaco { x: recuo, y, texto: numero, estilo, tamanho: CORPO, cor: COR_TEXTO, ..Pedaco::default() });
        }
        let x = recuo + if pedacos.is_empty() { 0.0 } else { coluna };
        let texto = cortar(&titulo, estilo, CORPO, limite - x - if numeros { 3.0 * em } else { 0.0 });
        pedacos.push(Pedaco { x, y, texto, estilo, tamanho: CORPO, cor: COR_TEXTO, ..Pedaco::default() });
        // 4 = no jeito do LaTeX (o pontilhado na cor do texto, o número da página no estilo da linha).
        let marcas = u8::from(numeros) | if numeros && !secao { 2 } else { 0 } | 4;
        linhas.push(Linha { altura: altura + antes, pedacos, alvo: Some(e.id), sumario: marcas, ..Linha::default() });
    }
    LayoutBloco { linhas, antes: 3.5 * 0.43 * em, depois: 14.0, minimo_antes: 2, minimo_depois: 2, ..LayoutBloco::default() }
}

/// Um parágrafo de uma lista gerada (referências, índice): o texto, os recuos e o espaço antes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParagrafoGerado {
    pub texto: Vec<Trecho>,
    pub atrib: Atrib,
    pub antes: f32,
}

/// Uma lista gerada pelo motor (a bibliografia, o índice remissivo): o título e os parágrafos. Como o
/// sumário, não se edita à mão — o cursor passa por cima dela inteira.
/// A lista com o `titulo` em cima (vazio = sem título).
/// O título de uma lista gerada (sumário, referências, índice). No LaTeX ela é um `\section*`, então
/// com `secao` (o estilo do título de nível 1 vindo do `\titleformat`) fica igual aos títulos.
fn titulo_de_lista(titulo: &str, secao: Option<&Estilo>, (tamanho, bits, depois): (f32, u8, f32), limite: f32) -> Vec<Linha> {
    if titulo.is_empty() {
        return Vec::new();
    }
    let (tamanho, bits, cor, depois) = match secao {
        Some(e) => (e.tamanho, e.bits, if e.cor == 0 { COR_TEXTO } else { e.cor }, e.depois.unwrap_or(depois)),
        None => (tamanho, bits, COR_TEXTO, depois),
    };
    let mut linhas = texto_corrido(&[Trecho::simples(titulo)], tamanho, bits, cor, 0.0, limite);
    if let Some(l) = linhas.last_mut() {
        l.altura += depois;
    }
    linhas
}

pub fn lista_gerada(titulo: &str, secao: Option<&Estilo>, paragrafos: &[ParagrafoGerado], limite: f32) -> LayoutBloco {
    let mut linhas = titulo_de_lista(titulo, secao, (16.0, NEGRITO, 6.0), limite);
    for p in paragrafos {
        let mut ls = texto_formatado(&p.texto, &Base::novo(CORPO, 0, COR_TEXTO, 0.0, limite), &p.atrib);
        if let Some(l) = ls.first_mut() {
            l.altura += p.antes;
            l.base += p.antes;
            for x in &mut l.pedacos {
                x.y += p.antes;
            }
        }
        linhas.extend(ls);
    }
    if paragrafos.is_empty() {
        let p = Pedaco { x: 0.0, y: linha_de_base(18.0, CORPO), texto: "—".to_string(), estilo: ITALICO, tamanho: CORPO, cor: COR_CINZA, ..Pedaco::default() };
        linhas.push(Linha { altura: 18.0, pedacos: vec![p], ..Linha::default() });
    }
    LayoutBloco { linhas, antes: 6.0, depois: 14.0, minimo_antes: 2, minimo_depois: 2, ..LayoutBloco::default() }
}

/// Corta o texto com reticências para caber em `limite`.
fn cortar(texto: &str, estilo: u8, tamanho: f32, limite: f32) -> String {
    if largura(texto, estilo, tamanho) <= limite {
        return texto.to_string();
    }
    let mut saida = String::new();
    for c in texto.chars() {
        if largura(&format!("{saida}{c}…"), estilo, tamanho) > limite {
            break;
        }
        saida.push(c);
    }
    saida.push('…');
    saida
}

/// O texto do título para o sumário.
pub fn texto_do_titulo(c: &Conteudo) -> Option<(u8, String)> {
    match c {
        Conteudo::Titulo { nivel, texto } => Some((*nivel, texto_puro(texto))),
        _ => None,
    }
}

/// O corpo padrão do tipo de bloco, em pontos.
pub fn tamanho_base(c: &Conteudo) -> f32 {
    match c {
        Conteudo::Titulo { nivel, .. } => TAMANHOS_TITULO[usize::from((*nivel).clamp(1, 6)) - 1],
        Conteudo::Codigo { .. } => 9.5,
        Conteudo::Tabela { .. } => 10.0,
        _ => CORPO,
    }
}

/// Compõe um bloco (menos o sumário, que depende dos outros: ver `sumario`).
pub fn compor(c: &Conteudo, a: &Atrib, limite: f32) -> LayoutBloco {
    compor_em(c, a, limite, &Entorno::default())
}

/// Como `compor`, com o número do título e o estilo com nome do bloco.
pub fn compor_em(c: &Conteudo, a: &Atrib, limite: f32, en: &Entorno) -> LayoutBloco {
    compor_em_mesclada(c, a, limite, en, &[])
}

/// Compõe tabelas mescladas sem suprimir divisórias em outras linhas.
pub fn compor_em_mesclada(
    c: &Conteudo, a: &Atrib, limite: f32, en: &Entorno,
    mesclagens: &[MesclagemTabela],
) -> LayoutBloco {
    let mut lay = compor_bloco(c, a, limite, en, mesclagens);
    lay.encher = a.encher;
    let fluxo = a.mais.fluxo;
    lay.manter_com_proximo |= fluxo & MANTER_COM_PROXIMO != 0;
    lay.quebra_antes |= fluxo & QUEBRA_ANTES != 0;
    if fluxo & VIUVAS_LIVRES != 0 {
        (lay.minimo_antes, lay.minimo_depois) = (lay.minimo_antes.min(1), lay.minimo_depois.min(1));
    }
    if fluxo & LINHAS_JUNTAS != 0 {
        lay.minimo_antes = lay.linhas.len();
    }
    if matches!(c, Conteudo::Titulo { .. } | Conteudo::Paragrafo { .. } | Conteudo::Item { .. } | Conteudo::Citacao { .. } | Conteudo::Regua) {
        lay.antes = a.antes.unwrap_or(lay.antes);
        lay.depois = a.depois.unwrap_or(lay.depois);
    }
    lay
}

fn compor_bloco(c: &Conteudo, a: &Atrib, limite: f32, en: &Entorno, mesclagens: &[MesclagemTabela]) -> LayoutBloco {
    match c {
        Conteudo::Titulo { nivel, texto } => titulo(*nivel, texto, limite, a, en),
        Conteudo::Paragrafo { texto } => paragrafo(texto, limite, a, en),
        Conteudo::Item { nivel, texto, .. } => item(*nivel, texto, limite, a),
        Conteudo::Citacao { texto } => citacao(texto, limite, a),
        Conteudo::Codigo { codigo: fonte, .. } => codigo(fonte, limite),
        Conteudo::Tabela { cabecalho, linhas } => tabela(*cabecalho, linhas, limite, mesclagens, a),
        Conteudo::Imagem { origem, legenda, largura, altura } => imagem(origem, legenda, *largura, *altura, limite, en.numero),
        Conteudo::Equacao { latex } => equacao(latex, limite, en.numero),
        Conteudo::Regua => regua(limite, a),
        Conteudo::QuebraDePagina => quebra_de_pagina(limite),
        // shortcut: o documento tem uma coluna só, então a quebra de coluna leva à página seguinte (como no Word); trocar quando houver colunas.
        Conteudo::QuebraDeColuna => marca_de_quebra("quebra de coluna", limite, true),
        Conteudo::QuebraDeSecao { inicio } => quebra_de_secao(*inicio, limite),
        Conteudo::Sumario { .. } | Conteudo::Bibliografia { .. } | Conteudo::IndiceRemissivo => LayoutBloco::default(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn par(texto: &str) -> LayoutBloco {
        compor(&Conteudo::Paragrafo { texto: vec![Trecho::simples(texto)] }, &Atrib::default(), 200.0)
    }

    #[test]
    fn letra_e_entrelinha_do_word_desenhadas() {
        use crate::modelo::{Fmt, MaisLetra, MaisParagrafo};
        let com = |mais: MaisLetra, estilo: u8| Trecho { texto: "Straße".into(), estilo, fmt: Fmt { mais, ..Fmt::default() }, ..Trecho::default() };
        let linha = |t: Trecho, a: &Atrib| compor(&Conteudo::Paragrafo { texto: vec![t] }, a, 200.0).linhas.remove(0);
        let caps = linha(com(MaisLetra { efeitos: MAIUSCULAS, ..MaisLetra::default() }, 0), &Atrib::default());
        assert_eq!(caps.pedacos[0].texto, "STRAßE", "o ß fica (viraria duas letras e o cursor se perderia)");
        let versalete = linha(com(MaisLetra { efeitos: VERSALETE, ..MaisLetra::default() }, 0), &Atrib::default());
        assert!(versalete.pedacos[0].tamanho < CORPO);
        let duplo = linha(com(MaisLetra { efeitos: TACHADO_DUPLO, ..MaisLetra::default() }, 0), &Atrib::default());
        assert_eq!(duplo.deco.iter().filter(|d| matches!(d, Deco::Traco { .. })).count(), 2);
        let onda = TIPOS_DE_SUBLINHADO.iter().position(|t| *t == "wave").unwrap() as u8;
        let ondulado = linha(com(MaisLetra { sublinhado: onda, ..MaisLetra::default() }, SUBLINHADO), &Atrib::default());
        assert!(ondulado.pedacos[0].estilo & SUBLINHADO == 0 && ondulado.deco.len() > 2, "a onda no lugar do traço reto");
        let exata = Atrib { mais: MaisParagrafo { entrelinha_exata: 30.0, ..MaisParagrafo::default() }, ..Atrib::default() };
        assert_eq!(linha(Trecho::simples("a"), &exata).altura, 30.0);
        let minima = Atrib { mais: MaisParagrafo { entrelinha_minima: 2.0, ..MaisParagrafo::default() }, ..Atrib::default() };
        assert_eq!(linha(Trecho::simples("a"), &minima).altura, linha(Trecho::simples("a"), &Atrib::default()).altura, "o mínimo menor não muda");
    }

    #[test]
    fn imagem_no_meio_da_linha_ocupa_a_largura_e_sobe_a_linha() {
        let imagem = Trecho::objeto(Objeto::Imagem { origem: "x.svg".into(), largura: 10.0, altura: 30.0 });
        let l = compor(&Conteudo::Paragrafo { texto: vec![Trecho::simples("A "), imagem, Trecho::simples(" B")] }, &Atrib::default(), 200.0);
        let linha = &l.linhas[0];
        let Some(Deco::Imagem { x, y, w, h, .. }) = linha.deco.first() else { panic!("{linha:?}") };
        assert_eq!((*w, *h), (10.0, 30.0));
        assert!((y + h - linha.base).abs() < 0.01 && *y >= 0.0, "em pé na linha de base, dentro da linha");
        let b = linha.pedacos.iter().find(|p| p.texto.contains('B')).unwrap();
        assert!(b.x >= x + w, "o texto depois vem depois da imagem");
    }

    #[test]
    fn tabulacao_encosta_o_resto_na_direita_e_desce_inteira_se_nao_cabe() {
        let l = par("Curso\tJan 2020 – Dez 2024");
        assert_eq!(l.linhas.len(), 1);
        let ultimo = l.linhas[0].pedacos.last().unwrap();
        assert!((ultimo.x + largura(&ultimo.texto, ultimo.estilo, ultimo.tamanho) - 200.0).abs() < 0.01, "{ultimo:?}");
        let l = par(&format!("{}\tJan 2020 – Dez 2024", "palavra ".repeat(3)));
        assert_eq!(l.linhas.len(), 2, "a data desce inteira");
        assert_eq!(l.linhas[1].pedacos.iter().map(|p| p.texto.as_str()).collect::<String>(), "Jan 2020 – Dez 2024");
    }

    #[test]
    fn justificado_parte_a_palavra_com_hifen_desenhado_fora_do_texto() {
        let texto = "Uma linha com espaço sobrando e desenvolvimento";
        let limite = largura("Uma linha com espaço sobrando e desenvol-", 0, CORPO) + 1.0;
        let a = Atrib { alinhamento: Alinhamento::Justificado, ..Atrib::default() };
        let l = compor(&Conteudo::Paragrafo { texto: vec![Trecho::simples(texto)] }, &a, limite);
        assert_eq!(l.linhas.len(), 2);
        assert!(matches!(&l.linhas[0].deco[..], [Deco::Texto { texto, .. }] if texto == "-"), "{:?}", l.linhas[0].deco);
        // O texto do parágrafo não ganha o hífen: o cursor e o arquivo não mudam.
        let tudo: String = l.linhas.iter().flat_map(|l| &l.pedacos).map(|p| p.texto.as_str()).collect();
        assert_eq!(tudo.replace(' ', ""), texto.replace(' ', ""));
        // Alinhado à esquerda não parte.
        let l = compor(&Conteudo::Paragrafo { texto: vec![Trecho::simples(texto)] }, &Atrib::default(), limite);
        assert!(l.linhas[0].deco.is_empty());
    }

    #[test]
    fn quebra_do_tex_no_paragrafo_inteiro_cabe_e_nao_perde_texto() {
        let texto = "Essa unidade curricular será ministrada com aulas expositivas-dialogadas e práticas em laboratório sob a supervisão do professor. As atividades empregarão a metodologia de aprendizado baseado em projetos, em que desafios são lançados e o docente orienta os estudantes.\u{2028}Depois da quebra.";
        let texto = texto.replace('\u{2028}', &QUEBRA_DE_LINHA.to_string());
        let a = Atrib { alinhamento: Alinhamento::Justificado, ..Atrib::default() };
        composicao_tex(true);
        let l = compor(&Conteudo::Paragrafo { texto: vec![Trecho::simples(&texto)] }, &a, 300.0);
        composicao_tex(false);
        assert!(l.linhas.len() > 3);
        for linha in &l.linhas {
            let p = linha.pedacos.iter().rev().find(|p| !p.texto.trim().is_empty()).unwrap();
            assert!(p.x + largura(&p.texto, p.estilo, p.tamanho) <= 300.01, "{p:?}");
        }
        assert!(l.linhas.last().unwrap().pedacos.iter().any(|p| p.texto.contains("Depois")), "a quebra forçada abre a última linha");
        let tudo: String = l.linhas.iter().flat_map(|l| &l.pedacos).map(|p| p.texto.as_str()).collect();
        assert_eq!(tudo.replace([' ', QUEBRA_DE_LINHA], ""), texto.replace([' ', QUEBRA_DE_LINHA], ""));
    }

    #[test]
    fn justificado_encolhe_os_espacos_para_caber_mais_uma_palavra_sem_estourar() {
        let a = Atrib { alinhamento: Alinhamento::Justificado, ..Atrib::default() };
        // Cabe "um dois três" só com os espaços encolhidos.
        let limite = largura("um dois três", 0, CORPO) - largura(" ", 0, CORPO) * 0.5;
        for texto in ["um dois três", "um dois três quatro"] {
            let l = compor(&Conteudo::Paragrafo { texto: vec![Trecho::simples(texto)] }, &a, limite);
            assert_eq!(l.linhas[0].pedacos.iter().map(|p| p.texto.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>(), ["um", "dois", "três"]);
            let p = l.linhas[0].pedacos.last().unwrap();
            assert!(p.x + largura(&p.texto, p.estilo, p.tamanho) <= limite + 0.01, "{p:?}");
        }
    }

    #[test]
    fn quebra_em_varias_linhas_sem_estourar() {
        let l = par(&"palavra ".repeat(40));
        assert!(l.linhas.len() > 3);
        for linha in &l.linhas {
            for p in &linha.pedacos {
                assert!(p.x + largura(&p.texto, p.estilo, p.tamanho) <= 200.01);
            }
        }
    }

    #[test]
    fn palavra_gigante_e_partida() {
        let l = par(&"x".repeat(300));
        assert!(l.linhas.len() > 1);
    }

    #[test]
    fn paragrafo_vazio_ocupa_uma_linha() {
        assert_eq!(par("").linhas.len(), 1);
    }

    #[test]
    fn tabela_tem_uma_linha_por_linha() {
        let cel = |t: &str| vec![Trecho::simples(t)];
        let l = compor(&Conteudo::Tabela { cabecalho: true, linhas: vec![vec![cel("a"), cel("b")], vec![cel("c"), cel("d")]] }, &Atrib::default(), 200.0);
        assert_eq!(l.linhas.len(), 2);
    }

    fn numeros_da_equacao(l: &LayoutBloco) -> Vec<String> {
        l.linhas[0].deco.iter().filter_map(|d| match d {
            Deco::Texto { texto, .. } => Some(texto.clone()),
            _ => None,
        }).collect()
    }

    #[test]
    fn equacao_em_destaque_e_composta_e_numerada() {
        let c = Conteudo::Equacao { latex: "\\begin{align} a &= b \\\\ c &= d \\nonumber \\\\ e &= \\frac{1}{2} \\end{align}".into() };
        let l = compor_em(&c, &Atrib::default(), 400.0, &Entorno { numero: Some("4"), estilo: None });
        assert_eq!(numeros_da_equacao(&l), vec!["\\text{(4)}", "\\text{(5)}"]);
        let p = &l.linhas[0].pedacos[0];
        assert_eq!(p.estilo, MATEMATICA | EXIBIDA);
        // Três linhas de fórmula: a altura é a da fórmula composta, não um número fixo.
        let f = feather_math::compor(&p.texto, feather_math::Modo::Exibida, CORPO);
        assert!(l.linhas[0].altura >= f.altura + f.profundidade);
        assert!(l.linhas[0].altura > 3.0 * CORPO);
        // Centrada e com o número na margem direita.
        assert!((p.x - (400.0 - f.largura) / 2.0).abs() < 0.01);
        let sem = compor(&Conteudo::Equacao { latex: "x^2".into() }, &Atrib::default(), 400.0);
        assert!(numeros_da_equacao(&sem).is_empty(), "\\[ … \\] não leva número");
    }

    #[test]
    fn formula_em_linha_e_uma_ficha_so_e_alarga_a_linha() {
        let t = vec![Trecho::simples("antes "), Trecho { texto: "\\frac{\\frac{a}{b}}{\\frac{c}{d}}".into(), estilo: MATEMATICA, ..Trecho::default() }, Trecho::simples(" depois")];
        let l = compor(&Conteudo::Paragrafo { texto: t }, &Atrib::default(), 400.0);
        let f = l.linhas[0].pedacos.iter().find(|p| p.estilo & MATEMATICA != 0).expect("a fórmula");
        assert!(f.texto.starts_with("\\frac"), "não se parte nas chaves nem nos espaços");
        let normal = par("antes depois");
        assert!(l.linhas[0].altura > normal.linhas[0].altura, "a fração empilhada aumenta a linha");
        assert!((largura(&f.texto, f.estilo, f.tamanho) - feather_math::largura(&f.texto, feather_math::Modo::Linha, f.tamanho)).abs() < 0.001);
    }

    #[test]
    fn tabela_do_latex_tem_os_fios_e_a_largura_do_texto() {
        let c = |t: &str| vec![Trecho::simples(t)];
        let linhas = vec![vec![c("Item"), c("Qtd.")], vec![c("Inversor"), c("1")]];
        let a = Atrib { alinhamento: Alinhamento::Centro, ..Atrib::default() };
        let t = tabela_latex(&linhas, "@{}lr@{}\nT;M;B", 400.0, &[], &a);
        // Três fios (o de cima e o do meio na 1ª linha, o de baixo na última), nenhum `|`.
        let fios: Vec<_> = t.linhas.iter().flat_map(|l| &l.deco).filter(|d| matches!(d, Deco::Traco { .. })).collect();
        assert_eq!(fios.len(), 3);
        // Sem `@{}` nas pontas: o texto encosta no fio; a tabela é mais estreita que a página, no meio.
        let Deco::Traco { x1, x2, .. } = fios[0] else { unreachable!() };
        assert!(*x1 > 0.0 && (x1 + x2 - 400.0).abs() < 0.01 && x2 - x1 < 200.0);
        assert!((t.linhas[1].pedacos[0].x - x1).abs() < 0.01);
        // A coluna `r`: o "1" acaba no fim da tabela.
        let um = &t.linhas[1].pedacos[1];
        assert!((um.x + largura("1", um.estilo, um.tamanho) - x2).abs() < 0.01);
        assert_eq!(colunas_latex("|l|c|c|c|").2, vec![1, 1, 1, 1, 1]);
        // `p{3cm}`: a coluna tem 3 cm e o texto quebra nela.
        assert_eq!(colunas_latex("p{3cm}l").1, vec![Some(3.0 * 28.3465), None]);
        let t = tabela_latex(&[vec![c("palavra palavra palavra palavra palavra"), c("x")]], "p{2cm}l\n;", 400.0, &[], &Atrib::default());
        assert!(t.linhas[0].pedacos.iter().map(|p| p.y).fold(0.0, f32::max) > 13.6);
        // `{\Huge 41}`: a linha cresce para a letra caber acima da base, sem invadir o que vem antes.
        let grande = vec![Trecho { fmt: crate::modelo::Fmt { tamanho: 24.88, ..Default::default() }, ..Trecho::simples("41") }];
        let t = tabela_latex(&[vec![grande, c("x")]], "cc\n;", 400.0, &[], &Atrib::default());
        assert!(t.linhas[0].pedacos[0].y >= 0.72 * 24.88 && t.linhas[0].altura > 0.72 * 24.88 + 4.0, "{}", t.linhas[0].altura);
    }

    #[test]
    fn sumario_tem_uma_linha_por_titulo() {
        let e = vec![EntradaSumario { id: 1, nivel: 1, texto: "A".into() }, EntradaSumario { id: 2, nivel: 3, texto: "B".into() }];
        assert_eq!(sumario(&e, 2, (true, true, false), 300.0, "Sumário", None).linhas.len(), 2);
        // No jeito do LaTeX: o número e o título em pedaços separados, o título na coluna dele.
        let l = sumario_latex(&[EntradaSumario { id: 1, nivel: 2, texto: "2.1  Irradiação".into() }], 3, true, 300.0, "Sumário", None).linhas;
        assert_eq!((l[1].pedacos[0].texto.as_str(), l[1].pedacos[1].x), ("2.1", 1.5 * CORPO + 2.3 * CORPO));
        assert_eq!(l[1].sumario, 1 | 2 | 4);
    }
}
