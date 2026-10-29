//! A paginação: decide em que página cada linha cai. Determinística — o mesmo
//! documento dá sempre as mesmas páginas, e quem desenha só recebe o resultado.
//!
//! Regras: viúva e órfã (um parágrafo não deixa 1 linha sozinha no fim nem no
//! início de uma página), título com o próximo (um título nunca fica isolado no
//! fim da página), quebra forçada, e linhas de tabela inteiras (nunca partidas).

use std::sync::Arc;

use crate::layout::LayoutBloco;
use crate::modelo::{Atrib, Bloco, Conteudo, Lista, ListaDoWord, Numeracao, Vertical};

/// Um pedaço contíguo de um bloco que cabe numa página.
#[derive(Clone, Debug, PartialEq)]
pub struct Fatia {
    /// Posição do bloco no documento.
    pub bloco: usize,
    /// As linhas `[de, ate)` do bloco.
    pub de: usize,
    pub ate: usize,
    /// Topo da primeira linha, a partir do topo da área útil.
    pub y: f32,
    /// Da margem esquerda ao começo da coluna da fatia (0 na primeira).
    pub x: f32,
    pub altura: f32,
    /// O marcador da lista (•, 1.), só na primeira fatia do item.
    pub marcador: Option<String>,
}

/// Aponta uma nota composta: `layouts[bloco].linhas[linha].notas[nota]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefNota {
    pub bloco: usize,
    pub linha: usize,
    pub nota: usize,
}

/// Uma página: as fatias na ordem de cima para baixo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pagina {
    pub fatias: Vec<Fatia>,
    /// As notas de rodapé da página, na ordem (o quadro as empilha no pé, acima da margem).
    pub notas: Vec<RefNota>,
    /// Depois do último bloco: as notas de fim, com o topo de cada uma; `None` é o título "Notas".
    pub notas_fim: Vec<(f32, Option<RefNota>)>,
}

/// O espaço do traço que separa as notas de rodapé do texto (o vão em cima, o traço, o vão embaixo).
pub const ESPACO_DAS_NOTAS: f32 = 12.0;
/// A altura do título das notas de fim.
pub const ALTURA_DO_TITULO_DAS_NOTAS: f32 = 30.0;

/// O quanto as notas de rodapé de uma linha somam ao pé da página (com o traço, se são as primeiras).
fn notas_da_linha(l: &crate::layout::Linha, ja_tem: bool) -> f32 {
    let h: f32 = l.notas.iter().filter(|n| !n.fim).map(crate::layout::NotaComposta::altura).sum();
    if h > 0.0 && !ja_tem {
        h + ESPACO_DAS_NOTAS
    } else {
        h
    }
}

/// A letra de uma numeração alfabética: a, b … z, aa, ab …
fn letras(mut n: u32) -> String {
    let mut s = Vec::new();
    while n > 0 {
        n -= 1;
        s.push((b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    s.iter().rev().collect()
}

/// O número romano (maiúsculo) de `n`.
pub fn romano(mut n: u32) -> String {
    const TABELA: [(u32, &str); 13] = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut s = String::new();
    for (v, r) in TABELA {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

/// O texto do marcador de um item: o rótulo do LaTeX (`modelo`, ver [`rotulo_do_modelo`]), o estilo de
/// lista escolhido, ou o padrão do nível.
fn marcador(ordenado: bool, a: &Atrib, nivel: usize, contagem: u32, modelo: &str) -> String {
    if let Some(r) = rotulo_do_modelo(modelo, contagem) {
        return r;
    }
    match (a.lista, ordenado) {
        (Lista::Circulo, _) => "○".to_string(),
        (Lista::Quadrado, _) => "▪".to_string(),
        (Lista::Seta, _) => "►".to_string(),
        (Lista::Tarefa, _) => String::new(),
        (Lista::Alfabetica, _) => format!("{})", letras(contagem)),
        (Lista::Romana, _) => format!("{}.", romano(contagem)),
        (Lista::RomanaMinuscula, _) => format!("{}.", romano(contagem).to_lowercase()),
        (_, true) => format!("{contagem}."),
        (_, false) => ["•", "◦", "▪"][nivel % 3].to_string(),
    }
}

/// O rótulo de texto de um item vindo do LaTeX, guardado no `Bloco::latex` como `estilo|modelo`
/// (`b|Art. #oº`: `b` = negrito, `r` = continua a contagem, `d` = rótulo alinhado à direita). No modelo, `#1` é o número, `#a`/`#A` a letra, `#i`/`#I` o romano e
/// `#o` o ordinal da legislação (1º até 9º, depois 10, 11…).
pub fn rotulo_do_modelo(modelo: &str, contagem: u32) -> Option<String> {
    let (_, m) = modelo.split_once('|')?;
    let ordinal = if contagem < 10 { format!("{contagem}º") } else { contagem.to_string() };
    Some(m.replace("#1", &contagem.to_string()).replace("#a", &letras(contagem)).replace("#A", &letras(contagem).to_uppercase()).replace("#i", &romano(contagem).to_lowercase()).replace("#I", &romano(contagem)).replace("#o", &ordinal))
}

/// Um número no formato do Word (`w:numFmt`).
fn numero_no_formato(n: u32, formato: &str) -> String {
    match formato {
        "lowerLetter" => letras(n),
        "upperLetter" => letras(n).to_uppercase(),
        "lowerRoman" => romano(n).to_lowercase(),
        "upperRoman" => romano(n),
        "decimalZero" => format!("{n:02}"),
        "none" | "bullet" => String::new(),
        _ => n.to_string(),
    }
}

/// O caractere que um marcador da Wingdings ou da Symbol (no trecho privado U+F0xx, ou o código de
/// 8 bits) desenha, nos caracteres que as fontes do motor têm; o resto vira "•".
fn marcador_de_simbolo(texto: &str, fonte: &str) -> String {
    let f = fonte.to_lowercase();
    let simbolica = f.contains("wingdings") || f.contains("symbol");
    let codigo = |c: char| if ('\u{F000}'..='\u{F0FF}').contains(&c) { Some(c as u32 - 0xF000) } else if simbolica { Some(c as u32) } else { None };
    let mut cs = texto.chars();
    let (Some(c), None) = (cs.next(), cs.next()) else { return texto.to_string() };
    if f.contains("courier") && c == 'o' {
        return "◦".to_string();
    }
    let Some(k) = codigo(c) else { return texto.to_string() };
    let wingdings = f.contains("wingdings") || !f.contains("symbol") && k != 0xB7;
    match (wingdings, k) {
        (true, 0xA7 | 0x6E) => "▪",
        (true, 0xD8 | 0xE0 | 0xE8) => "►",
        // ♦, não ◆: o ◆ (U+25C6) não existe nas Liberation que o motor usa.
        (true, 0x76 | 0x75) => "♦",
        (true, 0x6C) => "●",
        (true, 0xA8 | 0x71 | 0x6F) => "□",
        _ => "•",
    }
    .to_string()
}

/// O rótulo de um nível de lista do Word: o `w:lvlText` com cada `%k` trocado pela contagem do nível
/// `k` no formato dele (o nível que ainda não contou mostra o início).
fn rotulo_do_word(lista: &ListaDoWord, nivel: usize, contagens: &[Option<u32>; 9]) -> String {
    let Some(n) = lista.niveis.get(nivel) else { return String::new() };
    if n.formato == "bullet" {
        return marcador_de_simbolo(&n.texto, &n.fonte);
    }
    let mut s = n.texto.clone();
    for k in (1..=9usize).rev() {
        let marca = format!("%{k}");
        if s.contains(&marca) {
            let nk = lista.niveis.get(k - 1).cloned().unwrap_or_default();
            let c = contagens[k - 1].unwrap_or(nk.inicio);
            s = s.replace(&marca, &numero_no_formato(c, &nk.formato));
        }
    }
    s
}

/// O marcador de cada bloco (só os itens de lista têm). `listas` são as do Word
/// (`Estilos::listas`): o item que aponta uma delas conta por ela, atravessando parágrafos.
pub fn marcadores(blocos: &[Bloco], listas: &[ListaDoWord]) -> Vec<Option<String>> {
    let mut contadores = [0u32; 9];
    // A contagem de cada lista do Word, por nível (`None` = o nível ainda não contou).
    // shortcut: conta por lista (`abstractNum`), não por `w:num`: o item de um `w:num` antigo depois de um reinício segue do reinício; separar se aparecer documento assim.
    let mut do_word: std::collections::HashMap<u16, [Option<u32>; 9]> = std::collections::HashMap::new();
    // As listas com `resume` (rótulo do LaTeX com `r`): a contagem segue de uma lista para a outra.
    let mut seguidas: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    blocos
        .iter()
        .map(|b| match &b.conteudo {
            // A tarefa continua com a caixa de marcar do FeatherDocs.
            Conteudo::Item { ordenado, nivel, .. } if b.latex.is_empty() && b.atrib.lista != Lista::Tarefa && listas.get(usize::from(b.atrib.mais.lista).wrapping_sub(1)).is_some() => {
                let lista = &listas[usize::from(b.atrib.mais.lista) - 1];
                let n = usize::from((*nivel).min(8));
                let c = do_word.entry(b.atrib.mais.lista).or_insert([None; 9]);
                let inicio = lista.niveis.get(n).map_or(1, |l| l.inicio);
                c[n] = Some(match (b.atrib.mais.inicio, c[n]) {
                    (Some(v), _) => v,
                    (None, Some(v)) if !b.atrib.reiniciar => v + 1,
                    _ => inicio,
                });
                for x in &mut c[n + 1..] {
                    *x = None;
                }
                if lista.niveis.get(n).is_none() {
                    return Some(marcador(*ordenado, &b.atrib, n, c[n].unwrap_or(1), ""));
                }
                Some(rotulo_do_word(lista, n, c))
            }
            Conteudo::Item { ordenado, nivel, .. } => {
                let n = usize::from((*nivel).min(8));
                for c in &mut contadores[n + 1..] {
                    *c = 0;
                }
                if b.atrib.reiniciar {
                    contadores[n] = 0;
                }
                if let (Some(v), true) = (b.atrib.mais.inicio, *ordenado) {
                    contadores[n] = v;
                } else if *ordenado {
                    contadores[n] += 1;
                } else {
                    contadores[n] = 0;
                }
                if b.latex.split_once('|').is_some_and(|(e, _)| e.contains('r')) {
                    let c = seguidas.entry(b.latex.as_str()).or_insert(0);
                    *c += 1;
                    return Some(marcador(*ordenado, &b.atrib, n, *c, &b.latex));
                }
                Some(marcador(*ordenado, &b.atrib, n, contadores[n], &b.latex))
            }
            // Um parágrafo recuado no meio da lista (a continuação de um item) não interrompe a contagem.
            _ if b.atrib.recuo_esq >= 18.0 => None,
            _ => {
                contadores = [0; 9];
                None
            }
        })
        .collect()
}

/// O número de cada título com a numeração ligada (1, 1.1, 1.1.1…; "Capítulo 1" no nível 1 do esquema
/// de capítulos). Um título com `sem_numero` não conta; um nível pulado aparece como 0, como no Word.
pub fn numeros_dos_titulos(blocos: &[Bloco], numeracao: Numeracao, capitulo: &str) -> Vec<Option<String>> {
    numeros(blocos, numeracao, capitulo, &["Figura", "Tabela", "Equação"])
}

/// Como `numeros_dos_titulos`, e também as legendas ("Figura 1 –", "Tabela 2 –"), que contam
/// sempre, cada tipo com o seu contador; `rotulos` são as palavras de figura, tabela e equação.
pub fn numeros<S: AsRef<str>>(blocos: &[Bloco], numeracao: Numeracao, capitulo: &str, rotulos: &[S]) -> Vec<Option<String>> {
    let mut contadores = [0u32; 6];
    let mut apendice = false;
    let mut legendas = [0u32; 3];
    let mut equacoes = 0u32;
    blocos
        .iter()
        .map(|b| {
            // A equação numerada guarda o primeiro número dela; o `align` de 3 linhas consome 3.
            if let Conteudo::Equacao { latex } = &b.conteudo {
                let n = feather_math::quantos_numeros(latex) as u32;
                if n == 0 {
                    return None;
                }
                equacoes += n;
                return Some((equacoes - n + 1).to_string());
            }
            // A imagem do LaTeX com a legenda embaixo dela ("Figura 1: …") conta como figura.
            let imagem_numerada = matches!(&b.conteudo, Conteudo::Imagem { legenda, .. } if !legenda.is_empty()) && matches!(b.latex.as_str(), ":" | ".");
            if imagem_numerada || (1..=3).contains(&b.atrib.legenda) && matches!(b.conteudo, Conteudo::Paragrafo { .. }) {
                let k = if imagem_numerada { 0 } else { usize::from(b.atrib.legenda - 1) };
                legendas[k] += 1;
                let separador = if matches!(b.latex.as_str(), ":" | ".") { b.latex.as_str() } else { " –" };
                return Some(format!("{} {}{separador}", rotulos.get(k).map_or("", AsRef::as_ref), legendas[k]));
            }
            if numeracao == Numeracao::Nenhuma {
                return None;
            }
            let Conteudo::Titulo { nivel, .. } = b.conteudo else { return None };
            if b.atrib.sem_numero {
                return None;
            }
            if b.atrib.apendice && !apendice {
                apendice = true;
                contadores[0] = 0;
            }
            let n = usize::from(nivel.clamp(1, 6)) - 1;
            contadores[n] += 1;
            for c in &mut contadores[n + 1..] {
                *c = 0;
            }
            let mut partes: Vec<String> = contadores[..=n].iter().map(u32::to_string).collect();
            if apendice {
                partes[0] = letra(contadores[0]);
            }
            Some(if numeracao == Numeracao::Capitulo && n == 0 { format!("{capitulo} {}", partes[0]) } else { partes.join(".") })
        })
        .collect()
}

/// 1 → A, 26 → Z, 27 → AA, como o `\Alph` do LaTeX estendido.
pub fn letra(mut n: u32) -> String {
    let mut s = String::new();
    while n > 0 {
        n -= 1;
        s.insert(0, char::from(b'A' + (n % 26) as u8));
        n /= 26;
    }
    s
}

struct Corrida<'a> {
    layouts: &'a [Arc<LayoutBloco>],
    marcadores: &'a [Option<String>],
    altura: f32,
    paginas: Vec<Pagina>,
    atual: Pagina,
    y: f32,
    depois_anterior: f32,
    anterior_foi_item: bool,
    /// Terminou numa quebra de página: a página de depois existe mesmo vazia (o cursor vai para ela).
    quebrou_no_fim: bool,
    /// A altura que as notas de rodapé já tomam no pé da página atual.
    notas: f32,
    /// As colunas de texto: quantas, a distância de uma ao começo da outra, a atual e a fatia em que ela começou.
    colunas: usize,
    passo: f32,
    coluna: usize,
    inicio_coluna: usize,
}

/// Quantas linhas de `linhas` cabem em `restante`, e a altura delas.
fn cabem(linhas: &[crate::layout::Linha], restante: f32) -> (usize, f32) {
    let mut k = 0;
    let mut soma = 0.0;
    for l in linhas {
        if soma + l.altura > restante + 0.01 {
            break;
        }
        soma += l.altura;
        k += 1;
    }
    (k, soma)
}

impl Corrida<'_> {
    /// A coluna atual ainda não tem nada (com uma coluna, a página).
    fn vazia(&self) -> bool {
        self.atual.fatias.len() <= self.inicio_coluna
    }

    fn nova_pagina(&mut self) {
        self.encher();
        let antiga = std::mem::take(&mut self.atual);
        self.paginas.push(antiga);
        self.y = 0.0;
        self.notas = 0.0;
        self.depois_anterior = 0.0;
        self.anterior_foi_item = false;
        self.coluna = 0;
        self.inicio_coluna = 0;
    }

    /// O texto não cabe mais: segue na coluna seguinte ou, na última, na página seguinte.
    fn avancar(&mut self) {
        if self.coluna + 1 >= self.colunas {
            self.nova_pagina();
            return;
        }
        self.coluna += 1;
        self.inicio_coluna = self.atual.fatias.len();
        self.y = 0.0;
        self.depois_anterior = 0.0;
        self.anterior_foi_item = false;
    }

    /// Reparte o que sobra no pé da página entre os blocos com espaço elástico antes (o `\vfill`).
    /// No topo da página ele some, como no LaTeX.
    fn encher(&mut self) {
        if self.colunas > 1 {
            return;
        }
        let elasticas = self.atual.fatias.iter().skip(1).filter(|f| f.de == 0 && self.layouts[f.bloco].encher).count();
        let Some(ultima) = self.atual.fatias.last() else { return };
        if elasticas == 0 || !self.atual.notas_fim.is_empty() {
            return;
        }
        let passo = (self.altura - self.notas - ultima.y - ultima.altura).max(0.0) / elasticas as f32;
        let mut desloc = 0.0;
        for (k, f) in self.atual.fatias.iter_mut().enumerate() {
            if k > 0 && f.de == 0 && self.layouts[f.bloco].encher {
                desloc += passo;
            }
            f.y += desloc;
        }
    }

    /// Como `cabem`, contando o pé da página que as notas das linhas tomam.
    fn cabem_com_notas(&self, linhas: &[crate::layout::Linha], restante: f32) -> (usize, f32) {
        let (mut k, mut soma, mut notas) = (0, 0.0, self.notas);
        for l in linhas {
            let extra = notas_da_linha(l, notas > 0.0);
            if soma + l.altura + notas + extra > restante + 0.01 {
                break;
            }
            soma += l.altura;
            notas += extra;
            k += 1;
        }
        (k, soma)
    }

    /// Começa a próxima página por causa de uma quebra forçada.
    fn quebrar(&mut self) {
        self.nova_pagina();
        self.quebrou_no_fim = true;
    }

    /// O espaço antes do bloco: o maior entre o "depois" do anterior e o "antes" deste.
    fn espaco(&self, lay: &LayoutBloco) -> f32 {
        if self.vazia() {
            return 0.0;
        }
        let mut e = self.depois_anterior.max(lay.antes);
        if self.anterior_foi_item && lay.marcador_x.is_none() {
            e = e.max(6.0);
        }
        e
    }

    fn colocar(&mut self, i: usize, de: usize, ate: usize, gap: f32, consumido: f32) {
        self.quebrou_no_fim = false;
        let lay = &self.layouts[i];
        let marcador = if de == 0 { self.marcadores[i].clone() } else { None };
        for li in de..ate {
            let l = &lay.linhas[li];
            self.notas += notas_da_linha(l, self.notas > 0.0);
            self.atual.notas.extend(l.notas.iter().enumerate().filter(|(_, n)| !n.fim).map(|(k, _)| RefNota { bloco: i, linha: li, nota: k }));
        }
        let x = self.coluna as f32 * self.passo;
        self.atual.fatias.push(Fatia { bloco: i, de, ate, y: self.y + gap, x, altura: consumido, marcador });
        self.y += gap + consumido;
        self.depois_anterior = lay.depois;
        self.anterior_foi_item = lay.marcador_x.is_some();
    }

    /// O bloco seguinte ainda abre bem (pelo menos as primeiras linhas) depois de `y_fim`?
    fn proximo_cabe(&self, i: usize, y_fim: f32, depois: f32) -> bool {
        let Some(prox) = self.layouts.get(i + 1) else { return true };
        if prox.quebra_antes || prox.linhas.is_empty() {
            return true;
        }
        let n = prox.minimo_antes.clamp(1, prox.linhas.len());
        let (_, h) = cabem(&prox.linhas[..n], f32::MAX);
        y_fim + depois.max(prox.antes) + h <= self.altura - self.notas + 0.01
    }

    /// Coloca um bloco inteiro, partindo-o entre páginas se preciso.
    fn bloco(&mut self, i: usize) {
        let lay = Arc::clone(&self.layouts[i]);
        // A quebra de página antes do parágrafo (`w:pageBreakBefore`): ele começa a página seguinte.
        if lay.quebra_antes && !self.vazia() {
            self.nova_pagina();
        }
        let n = lay.linhas.len();
        let mut de = 0;
        while de < n {
            let gap = self.espaco(&lay);
            let (cabe, consumido) = self.cabem_com_notas(&lay.linhas[de..], self.altura - self.y - gap);
            let resta = n - de - cabe;
            if resta == 0 {
                let junto = de > 0 || !lay.manter_com_proximo || self.proximo_cabe(i, self.y + gap + consumido, lay.depois);
                if junto || self.vazia() {
                    self.colocar(i, de, n, gap, consumido);
                    if lay.quebra_coluna {
                        self.avancar();
                    }
                    if lay.quebra_depois {
                        self.quebrar();
                        // A página nova é a `paginas.len() + 1`; sem a paridade pedida, fica uma em branco no meio.
                        if lay.pagina_par.is_some_and(|par| (self.paginas.len() + 1).is_multiple_of(2) != par) {
                            self.nova_pagina();
                        }
                    }
                    return;
                }
                self.avancar();
                continue;
            }
            let k = self.quanto_cabe_antes_de_quebrar(&lay, de, cabe, resta);
            if k == 0 {
                self.avancar();
                continue;
            }
            let (_, h) = cabem(&lay.linhas[de..de + k], f32::MAX);
            self.colocar(i, de, de + k, gap, h);
            de += k;
            self.avancar();
        }
    }

    /// Quantas linhas ficam nesta página, respeitando órfã e viúva; 0 = o bloco vai todo para a próxima.
    fn quanto_cabe_antes_de_quebrar(&self, lay: &LayoutBloco, de: usize, cabe: usize, resta: usize) -> usize {
        let minimo = if de == 0 { lay.minimo_antes.max(1) } else { 1 };
        let mut k = cabe;
        if resta < lay.minimo_depois {
            k = k.saturating_sub(lay.minimo_depois - resta);
        }
        if k >= minimo {
            return k;
        }
        // Não dá para respeitar as regras: em página vazia força o que couber (ao menos uma linha).
        if self.vazia() {
            cabe.max(1)
        } else {
            0
        }
    }
}

impl Corrida<'_> {
    /// As notas de fim, depois do último bloco: o título e cada nota inteira (a que não cabe vai para a página seguinte).
    fn notas_de_fim(&mut self) {
        let refs: Vec<RefNota> = self
            .layouts
            .iter()
            .enumerate()
            .flat_map(|(i, lay)| lay.linhas.iter().enumerate().flat_map(move |(li, l)| l.notas.iter().enumerate().filter(|(_, n)| n.fim).map(move |(k, _)| RefNota { bloco: i, linha: li, nota: k })))
            .collect();
        let Some(primeira) = refs.first() else { return };
        let altura = |r: &RefNota| self.layouts[r.bloco].linhas[r.linha].notas[r.nota].altura();
        let mut y = self.y + if self.vazia() { 0.0 } else { 18.0 };
        if y + ALTURA_DO_TITULO_DAS_NOTAS + altura(primeira) > self.altura - self.notas && !self.vazia() {
            self.nova_pagina();
            y = 0.0;
        }
        self.atual.notas_fim.push((y, None));
        y += ALTURA_DO_TITULO_DAS_NOTAS;
        for r in refs {
            let h = altura(&r);
            if y + h > self.altura - self.notas && y > 0.0 {
                self.nova_pagina();
                y = 0.0;
            }
            self.atual.notas_fim.push((y, Some(r)));
            y += h + 2.0;
        }
        self.y = y;
        self.quebrou_no_fim = false;
    }

    fn pagina_tem_algo(&self) -> bool {
        !self.atual.fatias.is_empty() || !self.atual.notas_fim.is_empty()
    }
}

/// Desce o texto de cada página para o centro ou para a base da área útil (o que sobra fica em cima).
pub fn alinhar_na_vertical(paginas: &mut [Pagina], altura: f32, v: Vertical) {
    if v == Vertical::Topo {
        return;
    }
    for p in paginas {
        let usado = p.fatias.last().map_or(0.0, |f| f.y + f.altura);
        let sobra = (altura - usado).max(0.0);
        let desloc = if v == Vertical::Centro { sobra / 2.0 } else { sobra };
        for f in &mut p.fatias {
            f.y += desloc;
        }
    }
}

/// Reparte os blocos compostos em páginas de altura útil `altura`, numa coluna.
pub fn paginar(layouts: &[Arc<LayoutBloco>], marcadores: &[Option<String>], altura: f32) -> Vec<Pagina> {
    paginar_em_colunas(layouts, marcadores, altura, 1, 0.0)
}

/// Como `paginar`, em `colunas` colunas; `passo` vai do começo de uma ao da seguinte.
pub fn paginar_em_colunas(layouts: &[Arc<LayoutBloco>], marcadores: &[Option<String>], altura: f32, colunas: usize, passo: f32) -> Vec<Pagina> {
    let mut c = Corrida { layouts, marcadores, altura, paginas: Vec::new(), atual: Pagina::default(), y: 0.0, depois_anterior: 0.0, anterior_foi_item: false, quebrou_no_fim: false, notas: 0.0, colunas: colunas.max(1), passo, coluna: 0, inicio_coluna: 0 };
    for i in 0..layouts.len() {
        c.bloco(i);
    }
    c.notas_de_fim();
    c.encher();
    if c.pagina_tem_algo() || c.paginas.is_empty() || c.quebrou_no_fim {
        let ultima = std::mem::take(&mut c.atual);
        c.paginas.push(ultima);
    }
    c.paginas
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::layout::Linha;
    use crate::modelo::Trecho;

    fn bloco(n: usize, h: f32) -> Arc<LayoutBloco> {
        let linhas = (0..n).map(|_| Linha { altura: h, ..Linha::default() }).collect();
        Arc::new(LayoutBloco { linhas, minimo_antes: 2, minimo_depois: 2, ..LayoutBloco::default() })
    }

    fn rodar(blocos: Vec<Arc<LayoutBloco>>, altura: f32) -> Vec<Pagina> {
        let m = vec![None; blocos.len()];
        paginar(&blocos, &m, altura)
    }

    #[test]
    fn duas_colunas_enchem_a_primeira_antes_da_segunda_e_a_quebra_de_coluna_pula() {
        // 30 linhas de 10 numa página de 100: 10 na coluna 1, 10 na 2, 10 na página seguinte.
        let m = vec![None];
        let p = paginar_em_colunas(&[bloco(30, 10.0)], &m, 100.0, 2, 250.0);
        assert_eq!(p.len(), 2);
        let xs: Vec<(f32, usize)> = p[0].fatias.iter().map(|f| (f.x, f.ate - f.de)).collect();
        assert_eq!(xs, vec![(0.0, 10), (250.0, 10)]);
        assert_eq!((p[1].fatias[0].x, p[1].fatias[0].y), (0.0, 0.0));
        // A quebra de coluna leva o bloco seguinte ao topo da coluna 2, na mesma página.
        let quebra = Arc::new(LayoutBloco { quebra_coluna: true, ..(*bloco(1, 10.0)).clone() });
        let m = vec![None; 3];
        let p = paginar_em_colunas(&[bloco(2, 10.0), quebra, bloco(2, 10.0)], &m, 100.0, 2, 250.0);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].fatias[2].x, p[0].fatias[2].y), (250.0, 0.0));
    }

    #[test]
    fn imagem_do_latex_com_legenda_conta_como_figura() {
        let img = |latex: &str| Bloco {
            id: 0,
            conteudo: Conteudo::Imagem { origem: "a.png".into(), legenda: vec![Trecho::simples("Logo")], largura: 0.0, altura: 0.0 },
            atrib: Atrib::default(),
            mesclagens: Vec::new(),
            latex: latex.into(),
            tabela: Default::default(),
            imagem: Default::default(),
        };
        let n = numeros_dos_titulos(&[img(":"), img("")], Numeracao::Nenhuma, "");
        assert_eq!(n, vec![Some("Figura 1:".to_string()), None]);
    }

    #[test]
    fn quebra_antes_linhas_juntas_e_sem_viuvas_do_word() {
        use crate::layout::compor;
        use crate::modelo::{MaisParagrafo, LINHAS_JUNTAS, QUEBRA_ANTES, VIUVAS_LIVRES};
        let com = |fluxo: u8| Atrib { mais: MaisParagrafo { fluxo, ..MaisParagrafo::default() }, ..Atrib::default() };
        let par = |a: &Atrib| Arc::new(compor(&Conteudo::Paragrafo { texto: vec![Trecho::simples("palavra ")] }, a, 200.0));
        let p = rodar(vec![par(&Atrib::default()), par(&com(QUEBRA_ANTES))], 500.0);
        assert_eq!((p.len(), p[1].fatias[0].bloco), (2, 1), "o segundo começa a página seguinte, com o texto");
        assert_eq!(par(&com(LINHAS_JUNTAS)).minimo_antes, 1);
        let longo = |fluxo: u8| compor(&Conteudo::Paragrafo { texto: vec![Trecho::simples(&"palavra ".repeat(80))] }, &com(fluxo), 200.0);
        assert_eq!(longo(LINHAS_JUNTAS).minimo_antes, longo(0).linhas.len());
        assert_eq!((longo(VIUVAS_LIVRES).minimo_antes, longo(VIUVAS_LIVRES).minimo_depois), (1, 1));
    }

    #[test]
    fn cabe_em_uma_pagina() {
        assert_eq!(rodar(vec![bloco(3, 10.0), bloco(3, 10.0)], 100.0).len(), 1);
    }

    #[test]
    fn viuva_e_orfa_deixam_duas_linhas() {
        // 10 linhas de 10: cabem 9 na primeira página (altura 95) → regra deixa 2 para a próxima (8 + 2).
        let p = rodar(vec![bloco(10, 10.0)], 95.0);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].fatias[0].ate, 8);
        assert_eq!(p[1].fatias[0].de, 8);
    }

    #[test]
    fn paragrafo_de_tres_linhas_nao_se_parte() {
        let p = rodar(vec![bloco(5, 10.0), bloco(3, 10.0)], 70.0);
        assert_eq!(p[1].fatias[0].bloco, 1);
        assert_eq!(p[1].fatias[0].ate - p[1].fatias[0].de, 3);
    }

    #[test]
    fn titulo_vai_junto_com_o_proximo() {
        let titulo = Arc::new(LayoutBloco { linhas: vec![Linha { altura: 10.0, ..Linha::default() }], manter_com_proximo: true, minimo_antes: 1, ..LayoutBloco::default() });
        let p = rodar(vec![bloco(4, 10.0), titulo, bloco(4, 10.0)], 60.0);
        // o título cabe no fim da página 1, mas o texto seguinte não: ele vai para a 2.
        assert!(p[1].fatias.iter().any(|f| f.bloco == 1));
    }

    fn quebra() -> Arc<LayoutBloco> {
        Arc::new(LayoutBloco { linhas: vec![Linha { altura: 10.0, ..Linha::default() }], minimo_antes: 1, quebra_depois: true, ..LayoutBloco::default() })
    }

    #[test]
    fn quebra_forcada_fica_no_fim_da_pagina_e_o_resto_vai_para_a_proxima() {
        let p = rodar(vec![bloco(1, 10.0), quebra(), bloco(1, 10.0)], 100.0);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].fatias.len(), 2, "o texto e a marca da quebra ficam na primeira");
        assert_eq!(p[1].fatias[0].bloco, 2);
    }

    #[test]
    fn vfill_reparte_o_que_sobra_na_pagina() {
        let elastico = || Arc::new(LayoutBloco { encher: true, ..(*bloco(1, 10.0)).clone() });
        // A capa: título, autor e data com `\vfill` antes dos dois últimos, e a quebra.
        let p = rodar(vec![elastico(), bloco(1, 10.0), elastico(), elastico(), quebra(), bloco(1, 10.0)], 100.0);
        let ys: Vec<f32> = p[0].fatias.iter().map(|f| f.y).collect();
        // Sobram 50 (5 linhas de 10): 25 antes de cada elástico; o do topo não conta.
        assert_eq!(ys, [0.0, 10.0, 45.0, 80.0, 90.0]);
        assert_eq!(p[1].fatias[0].y, 0.0);
    }

    #[test]
    fn quebra_no_fim_do_documento_deixa_uma_pagina_vazia_depois() {
        assert_eq!(rodar(vec![bloco(1, 10.0), quebra()], 100.0).len(), 2);
    }

    #[test]
    fn linha_maior_que_a_pagina_nao_trava() {
        assert_eq!(rodar(vec![bloco(1, 500.0)], 100.0).len(), 1);
    }

    fn com_nota(h_nota: f32, fim: bool) -> Arc<LayoutBloco> {
        let nota = crate::layout::NotaComposta { fim, linhas: vec![Linha { altura: h_nota, ..Linha::default() }], desl: 0, coluna: None };
        let linhas = vec![Linha { altura: 10.0, notas: vec![nota], ..Linha::default() }];
        Arc::new(LayoutBloco { linhas, minimo_antes: 1, minimo_depois: 1, ..LayoutBloco::default() })
    }

    #[test]
    fn nota_de_rodape_toma_o_pe_da_pagina() {
        // 5 linhas de 10 = 50; a nota de 20 (+2 de respiro +12 do traço) não cabe junto numa página de 80.
        let p = rodar(vec![bloco(5, 10.0), com_nota(20.0, false)], 80.0);
        assert_eq!(p.len(), 2, "a linha com a chamada vai para onde a nota cabe");
        assert_eq!(p[1].notas, vec![RefNota { bloco: 1, linha: 0, nota: 0 }]);
        let p = rodar(vec![bloco(2, 10.0), com_nota(20.0, false)], 80.0);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].notas.len(), 1);
    }

    #[test]
    fn notas_de_fim_vao_depois_do_ultimo_bloco() {
        let p = rodar(vec![com_nota(10.0, true), bloco(2, 10.0)], 200.0);
        assert!(p[0].notas.is_empty());
        assert_eq!(p[0].notas_fim.len(), 2, "o título e a nota");
        assert!(p[0].notas_fim[0].1.is_none());
        assert!(p[0].notas_fim[0].0 >= 30.0, "depois do texto");
    }

    #[test]
    fn numeracao_de_lista() {
        let it = |o, n| Bloco::novo(0, Conteudo::Item { ordenado: o, nivel: n, texto: vec![] });
        let m = marcadores(&[it(true, 0), it(true, 0), it(true, 1), it(true, 1), it(true, 0)], &[]);
        let m: Vec<String> = m.into_iter().flatten().collect();
        assert_eq!(m, vec!["1.", "2.", "1.", "2.", "3."]);
    }

    #[test]
    fn lista_do_word_comeca_reinicia_continua_e_usa_o_texto_do_nivel() {
        use crate::modelo::{ListaDoWord, NivelDeLista};
        let nivel = |formato: &str, texto: &str, inicio| NivelDeLista { formato: formato.into(), texto: texto.into(), inicio, ..NivelDeLista::default() };
        let listas = [
            ListaDoWord { niveis: vec![nivel("decimal", "Passo %1:", 5), nivel("lowerLetter", "%1.%2)", 1)], estilo: String::new() },
            ListaDoWord { niveis: vec![nivel("bullet", "\u{F0D8}", 1)].into_iter().map(|n| NivelDeLista { fonte: "Wingdings".into(), ..n }).collect(), estilo: String::new() },
        ];
        let it = |n, lista| {
            let mut b = Bloco::novo(0, Conteudo::Item { ordenado: lista == 1, nivel: n, texto: vec![] });
            b.atrib.mais.lista = lista;
            b
        };
        let p = Bloco::novo(0, Conteudo::Paragrafo { texto: vec![] });
        let mut reinicio = it(0, 1);
        reinicio.atrib.reiniciar = true;
        let mut em_dez = it(0, 1);
        em_dez.atrib.mais.inicio = Some(10);
        let m: Vec<String> = marcadores(&[it(0, 1), it(1, 1), it(1, 1), p.clone(), it(0, 1), it(0, 2), reinicio, em_dez, it(0, 1)], &listas).into_iter().map(Option::unwrap_or_default).collect();
        assert_eq!(m, ["Passo 5:", "5.a)", "5.b)", "", "Passo 6:", "►", "Passo 5:", "Passo 10:", "Passo 11:"]);
        // O item do FeatherDocs (sem lista do Word) recomeça depois do parágrafo e segue o `inicio`.
        let mut tres = Bloco::novo(0, Conteudo::Item { ordenado: true, nivel: 0, texto: vec![] });
        tres.atrib.mais.inicio = Some(3);
        let n: Vec<String> = marcadores(&[it(0, 0), p, tres, it(0, 0)].map(|mut b| { if let Conteudo::Item { ordenado, .. } = &mut b.conteudo { *ordenado = true; } b }), &[]).into_iter().map(Option::unwrap_or_default).collect();
        assert_eq!(n, ["1.", "", "3.", "4."]);
    }

    #[test]
    fn equacoes_numeradas_em_sequencia_pelo_documento() {
        let mut d = crate::modelo::Documento::default();
        d.blocos.clear();
        for latex in ["\\begin{equation} a \\end{equation}", "x", "\\begin{align} a \\\\ b \\\\ c \\nonumber \\end{align}", "\\begin{equation} z \\end{equation}"] {
            d.acrescentar(Conteudo::Equacao { latex: latex.into() });
        }
        let n = numeros(&d.blocos, Numeracao::Nenhuma, "Capítulo", &["Figura", "Tabela", "Equação"]);
        assert_eq!(n, vec![Some("1".into()), None, Some("2".into()), Some("4".into())]);
    }

    #[test]
    fn numeros_dos_titulos_decimal_capitulo_e_sem_numero() {
        let t = |nivel: u8| Bloco::novo(0, Conteudo::Titulo { nivel, texto: Vec::new() });
        let p = Bloco::novo(0, Conteudo::Paragrafo { texto: Vec::new() });
        let mut fora = t(2);
        fora.atrib.sem_numero = true;
        let blocos = [t(1), p.clone(), t(2), t(2), fora, t(3), t(1), t(3)];
        let n: Vec<String> = numeros_dos_titulos(&blocos, Numeracao::Decimal, "Capítulo").into_iter().map(Option::unwrap_or_default).collect();
        assert_eq!(n, ["1", "", "1.1", "1.2", "", "1.2.1", "2", "2.0.1"]);
        let c = numeros_dos_titulos(&blocos, Numeracao::Capitulo, "Capítulo");
        assert_eq!(c[0].as_deref(), Some("Capítulo 1"));
        assert_eq!(c[2].as_deref(), Some("1.1"));
        assert!(numeros_dos_titulos(&blocos, Numeracao::Nenhuma, "").iter().all(Option::is_none));
    }

    #[test]
    fn apendice_recomeca_em_letras() {
        let t = |nivel: u8| Bloco::novo(0, Conteudo::Titulo { nivel, texto: Vec::new() });
        let mut a = t(1);
        a.atrib.apendice = true;
        let blocos = [t(1), t(1), a, t(2), t(1)];
        let n: Vec<String> = numeros_dos_titulos(&blocos, Numeracao::Decimal, "").into_iter().map(Option::unwrap_or_default).collect();
        assert_eq!(n, ["1", "2", "A", "A.1", "B"]);
        assert_eq!((letra(26), letra(27)), ("Z".to_string(), "AA".to_string()));
    }
}
