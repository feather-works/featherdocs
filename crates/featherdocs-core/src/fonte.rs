//! Formatação de caractere além de negrito/itálico: família, tamanho, cor, realce,
//! sobrescrito/subscrito, maiúsculas, limpar formatação e o pincel. Tudo atua sobre a
//! seleção; com o cursor parado vale para o próximo texto digitado (`pendente`).

use crate::alvo::{com_texto_ref, eh_atomico, texto_alvo};
use crate::cursor::Pos;
use crate::digitacao::tem_texto_corrido;
use crate::formato::{faixas, Faixa};
use crate::layout::tamanho_base;
use crate::modelo::{Atrib, Conteudo, Fmt, MaisParagrafo, CODIGO, MATEMATICA, SERIFA};
use crate::sessao::{Mudanca, Sessao};
use crate::texto::{fatia, modificar, palavra_em, plano};

/// O que o pincel de formatação carrega: o caractere onde o cursor estava e o parágrafo dele.
#[derive(Clone, Copy)]
pub(crate) struct Pincel {
    estilo: u8,
    fmt: Fmt,
    atrib: Atrib,
}

/// Lê `#RRGGBB` (ou `#RRGGBBAA`); texto vazio é "automática" (0).
pub fn ler_cor(texto: &str) -> Result<u32, String> {
    let t = texto.trim().trim_start_matches('#');
    if t.is_empty() {
        return Ok(0);
    }
    let erro = || format!("Cor inválida: \"{texto}\" (use #RRGGBB).");
    let v = u32::from_str_radix(t, 16).map_err(|_| erro())?;
    match t.len() {
        6 => Ok((v << 8) | 0xff),
        8 => Ok(v),
        _ => Err(erro()),
    }
}

/// `0xRRGGBBAA` como `#rrggbb` (vazio se 0).
pub fn cor_em_texto(cor: u32) -> String {
    if cor == 0 {
        String::new()
    } else {
        format!("#{:06x}", cor >> 8)
    }
}

/// Os modos do menu "Maiúsculas e minúsculas".
#[derive(Clone, Copy, PartialEq)]
enum Caixa {
    Frase,
    Minusculas,
    Maiusculas,
    Palavras,
    Alternar,
}

fn modo_de_caixa(nome: &str) -> Result<Caixa, String> {
    Ok(match nome {
        "frase" => Caixa::Frase,
        "minusculas" => Caixa::Minusculas,
        "maiusculas" => Caixa::Maiusculas,
        "palavras" => Caixa::Palavras,
        "alternar" => Caixa::Alternar,
        _ => return Err(format!("Modo de maiúsculas desconhecido: \"{nome}\".")),
    })
}

/// Troca a caixa de `c` só se o resultado continua sendo um caractere (ß → SS ficaria de fora).
fn trocar(c: char, maiuscula: bool) -> char {
    let mut it: Vec<char> = if maiuscula { c.to_uppercase().collect() } else { c.to_lowercase().collect() };
    if it.len() == 1 {
        it.remove(0)
    } else {
        c
    }
}

/// Aplica o modo a `texto`; `inicio` diz se o caractere antes dele separa palavra/frase.
fn caixa_do_texto(texto: &str, modo: Caixa, inicio: &mut bool, frase: &mut bool) -> String {
    texto
        .chars()
        .map(|c| {
            let sai = match modo {
                Caixa::Minusculas => trocar(c, false),
                Caixa::Maiusculas => trocar(c, true),
                Caixa::Alternar => trocar(c, !c.is_uppercase()),
                Caixa::Palavras => trocar(c, *inicio),
                Caixa::Frase => trocar(c, *frase),
            };
            if c.is_alphanumeric() {
                *frase = false;
            } else if matches!(c, '.' | '!' | '?') {
                *frase = true;
            }
            *inicio = !c.is_alphanumeric() && c != '\'';
            sai
        })
        .collect()
}

impl Sessao {
    /// Muda `f(estilo, fmt)` em cada trecho da seleção; com o cursor parado, no que se digita a seguir.
    fn mudar_marca(&mut self, f: impl Fn(&mut u8, &mut Fmt)) -> Mudanca {
        if self.cursor.vazio() {
            let (mut e, mut fm) = self.marca_no_cursor();
            f(&mut e, &mut fm);
            self.cursor.pendente = Some((e, fm));
            return self.sem_mudanca();
        }
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        for p in faixas(&t, ini, fim) {
            com_texto_ref(&mut t.blocos[p.bloco].conteudo, p.celula, |x| {
                modificar(x, p.a, p.b, |tr| {
                    let (mut e, mut fm) = (tr.estilo, tr.fmt);
                    f(&mut e, &mut fm);
                    tr.estilo = e;
                    tr.fmt = fm;
                });
            });
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// A família: `sans` (padrão), `serifa` ou `mono`.
    pub fn fonte_definir(&mut self, familia: &str) -> Result<Mudanca, String> {
        let (tira, poe) = match familia {
            "sans" => (SERIFA | CODIGO, 0),
            "serifa" => (SERIFA | CODIGO, SERIFA),
            "mono" => (SERIFA | CODIGO, CODIGO),
            _ => return Err(format!("Não conheço a fonte \"{familia}\".")),
        };
        // A classe escolhida à mão troca a fonte com nome (vinda do Word) pela da classe.
        Ok(self.mudar_marca(|e, f| {
            *e = (*e & !tira) | poe;
            f.familia = crate::modelo::Familia::default();
        }))
    }

    /// O corpo em pontos (de 1 a 400); 0 volta ao do tipo de bloco.
    pub fn tamanho_definir(&mut self, pt: f32) -> Result<Mudanca, String> {
        if !(0.0..=400.0).contains(&pt) {
            return Err(format!("Tamanho de fonte inválido: {pt} (use de 1 a 400 pt)."));
        }
        let pt = if pt > 0.0 && pt < 1.0 { 1.0 } else { pt };
        Ok(self.mudar_marca(|_, f| f.tamanho = pt))
    }

    /// Aumenta (`+1`) ou diminui (`-1`) a fonte: passo de 1 pt até 12 e de 2 pt depois.
    pub fn tamanho_passo(&mut self, delta: i32) -> Mudanca {
        let atual = self.tamanho_no_cursor();
        let passo = |t: f32, sobe: bool| -> f32 {
            let p = if (sobe && t >= 12.0) || (!sobe && t > 12.0) { 2.0 } else { 1.0 };
            if sobe { t + p } else { t - p }
        };
        let novo = passo(atual, delta > 0).clamp(1.0, 400.0);
        self.mudar_marca(|_, f| f.tamanho = novo)
    }

    /// A formatação que a barra mostra: a do cursor, ou a do primeiro caractere da seleção.
    pub(crate) fn marca_da_selecao(&self) -> (u8, Fmt) {
        if self.cursor.vazio() {
            return self.marca_no_cursor();
        }
        let (ini, _) = self.selecao_ordenada();
        let t = self.idx(ini.bloco).and_then(|i| texto_alvo(&self.doc.blocos[i].conteudo, ini.celula));
        t.map_or((0, Fmt::default()), |t| crate::texto::marca_em(&t, ini.desl + 1))
    }

    /// O corpo efetivo no cursor (o do trecho ou o do tipo de bloco).
    pub(crate) fn tamanho_no_cursor(&self) -> f32 {
        let (_, f) = self.marca_da_selecao();
        if f.tamanho > 0.0 {
            return f.tamanho;
        }
        self.idx(self.cursor.foco.bloco).map_or(11.0, |i| {
            let b = &self.doc.blocos[i];
            self.doc.estilos.resolvido_do_bloco(b).filter(|e| e.tamanho > 0.0).map_or_else(|| tamanho_base(&b.conteudo), |e| e.tamanho)
        })
    }

    /// A cor do texto (`#RRGGBB`; vazio = automática).
    pub fn cor_definir(&mut self, cor: &str) -> Result<Mudanca, String> {
        let c = ler_cor(cor)?;
        Ok(self.mudar_marca(|_, f| f.cor = c))
    }

    /// A cor do marca-texto (`#RRGGBB`; vazio = sem realce).
    pub fn realce_definir(&mut self, cor: &str) -> Result<Mudanca, String> {
        let c = ler_cor(cor)?;
        Ok(self.mudar_marca(|_, f| f.realce = c))
    }

    /// Sobrescrito (`1`) ou subscrito (`-1`): liga, ou desliga se já estava assim.
    pub fn posicao_alternar(&mut self, p: i8) -> Mudanca {
        let ja = self.marca_da_selecao().1.posicao == p;
        self.mudar_marca(|_, f| f.posicao = if ja { 0 } else { p })
    }

    /// Tira a formatação de caractere (negrito, cor, tamanho, fonte…); o link e a fórmula ficam.
    pub fn limpar_formatacao(&mut self) -> Mudanca {
        self.mudar_marca(|e, f| {
            *e &= MATEMATICA;
            *f = Fmt::default();
        })
    }

    /// Maiúsculas e minúsculas: `frase`, `minusculas`, `maiusculas`, `palavras`, `alternar`.
    /// Sem seleção, vale para a palavra onde o cursor está.
    pub fn maiusculas(&mut self, modo: &str) -> Result<Mudanca, String> {
        let modo = modo_de_caixa(modo)?;
        if self.cursor.vazio() {
            self.selecionar_palavra_do_cursor();
        }
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        let mut mudou = false;
        for p in faixas(&t, ini, fim) {
            mudou |= trocar_caixa_da_faixa(&mut t.blocos[p.bloco].conteudo, &p, modo);
        }
        if !mudou {
            return Ok(self.sem_mudanca());
        }
        let cursor = self.cursor;
        Ok(self.fechar_trabalho(t, cursor, None))
    }

    fn selecionar_palavra_do_cursor(&mut self) {
        let f = self.cursor.foco;
        let Some(i) = self.idx(f.bloco) else { return };
        let Some(t) = texto_alvo(&self.doc.blocos[i].conteudo, f.celula) else { return };
        let chars: Vec<char> = plano(&t).chars().collect();
        let (a, b) = palavra_em(&chars, f.desl);
        if a < b && chars[a].is_alphanumeric() {
            self.cursor.ancora = Pos { desl: a, ..f };
            self.cursor.foco = Pos { desl: b, ..f };
        }
    }

    /// Guarda a formatação do cursor para o pincel.
    pub fn pincel_copiar(&mut self) {
        let (estilo, fmt) = self.marca_no_cursor();
        let atrib = self.idx(self.cursor.foco.bloco).map(|i| self.doc.blocos[i].atrib).unwrap_or_default();
        self.pincel = Some(Pincel { estilo, fmt, atrib });
    }

    /// O pincel tem formatação guardada?
    pub fn pincel_cheio(&self) -> bool {
        self.pincel.is_some()
    }

    /// Descarta o estilo capturado sem modificar a seleção ou o documento.
    pub fn pincel_cancelar(&mut self) {
        self.pincel = None;
    }

    /// Aplica a formatação guardada à seleção (caractere e parágrafo). Sem seleção, não faz nada.
    pub fn pincel_aplicar(&mut self) -> Mudanca {
        let Some(p) = self.pincel else { return self.sem_mudanca() };
        if self.cursor.vazio() {
            return self.sem_mudanca();
        }
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        for f in faixas(&t, ini, fim) {
            com_texto_ref(&mut t.blocos[f.bloco].conteudo, f.celula, |x| {
                modificar(x, f.a, f.b, |tr| {
                    tr.estilo = (p.estilo & !MATEMATICA) | (tr.estilo & MATEMATICA);
                    tr.fmt = p.fmt;
                });
            });
        }
        for b in t.blocos.iter_mut().filter(|b| tem_texto_corrido(&b.conteudo)) {
            copiar_paragrafo(&mut b.atrib, &p.atrib);
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }
}

/// O que do parágrafo o pincel leva (a lista e a numeração ficam).
fn copiar_paragrafo(para: &mut Atrib, de: &Atrib) {
    *para = Atrib { lista: para.lista, marcado: para.marcado, reiniciar: para.reiniciar, mais: MaisParagrafo { lista: para.mais.lista, inicio: para.mais.inicio, ..de.mais }, ..*de };
}

/// Troca a caixa do texto coberto pela faixa; `true` se algo mudou.
fn trocar_caixa_da_faixa(c: &mut Conteudo, f: &Faixa, modo: Caixa) -> bool {
    if eh_atomico(c) {
        return false;
    }
    let Some(original) = texto_alvo(c, f.celula) else { return false };
    let todo: Vec<char> = plano(&original).chars().collect();
    let anterior = if f.a == 0 { None } else { todo.get(f.a - 1).copied() };
    let mut inicio = anterior.map_or(true, |c| !c.is_alphanumeric());
    let mut frase = anterior.map_or(true, |c| c.is_whitespace()) && todo[..f.a].iter().rev().find(|c| !c.is_whitespace()).map_or(true, |c| matches!(c, '.' | '!' | '?'));
    let mut mudou = false;
    com_texto_ref(c, f.celula, |x| {
        let meio = fatia(x, f.a, f.b);
        let mut novo = Vec::with_capacity(meio.len());
        for mut tr in meio {
            let s = caixa_do_texto(&tr.texto, modo, &mut inicio, &mut frase);
            mudou |= s != tr.texto;
            tr.texto = s;
            novo.push(tr);
        }
        let antes = fatia(x, 0, f.a);
        let depois = fatia(x, f.b, usize::MAX);
        *x = crate::texto::juntar(crate::texto::juntar(antes, novo), depois);
    });
    mudou
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::cursor::Movimento;
    use crate::inline::analisar;
    use crate::modelo::Documento;

    fn doc(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        Sessao::com_documento(d)
    }

    fn selecionar(s: &mut Sessao, n: usize) {
        for _ in 0..n {
            s.mover(Movimento::Direita, true);
        }
    }

    fn texto0(s: &Sessao) -> Vec<crate::modelo::Trecho> {
        let Conteudo::Paragrafo { texto } = &s.documento().blocos[0].conteudo else { panic!() };
        texto.clone()
    }

    #[test]
    fn tamanho_cor_e_realce_na_selecao() {
        let mut s = doc(&["abcdef"]);
        selecionar(&mut s, 3);
        s.tamanho_definir(20.0).unwrap();
        s.cor_definir("#ff0000").unwrap();
        s.realce_definir("#ffff00").unwrap();
        let t = texto0(&s);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].fmt.tamanho, 20.0);
        assert_eq!(t[0].fmt.cor, 0xff00_00ff);
        assert_eq!(t[0].fmt.realce, 0xffff_00ff);
        assert!(t[1].fmt.eh_padrao());
        let linha = &s.layouts[0].linhas[0];
        assert!(linha.altura > 25.0, "a linha cresce com o corpo 20: {}", linha.altura);
        assert!(linha.pedacos.iter().any(|p| p.realce != 0));
        assert_eq!(s.formato_do_cursor().tamanho, 20.0);
        assert!(s.tamanho_definir(900.0).is_err());
        assert!(s.cor_definir("vermelho").is_err());
    }

    #[test]
    fn familia_serifa_mede_diferente_e_mono_vira_codigo() {
        let mut s = doc(&["Hello"]);
        let antes = s.layouts[0].linhas[0].pedacos[0].x;
        s.selecionar_tudo();
        s.fonte_definir("serifa").unwrap();
        assert_eq!(s.formato_do_cursor().fonte, "serifa");
        let w_serifa: f32 = s.layouts[0].linhas[0].pedacos.iter().map(|p| crate::medida::largura(&p.texto, p.estilo, p.tamanho)).sum();
        assert!((w_serifa - 24.44).abs() < 0.1, "Hello em Times 11 pt ≈ 24,4 pt, deu {w_serifa}");
        s.fonte_definir("mono").unwrap();
        assert_eq!(s.formato_do_cursor().fonte, "mono");
        s.fonte_definir("sans").unwrap();
        assert_eq!(s.documento().blocos[0].conteudo, Conteudo::Paragrafo { texto: analisar("Hello") });
        assert_eq!(antes, 0.0);
        assert!(s.fonte_definir("comic").is_err());
    }

    #[test]
    fn sobrescrito_alterna_e_encolhe() {
        let mut s = doc(&["x2"]);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Direita, false);
        selecionar(&mut s, 0);
        s.mover(Movimento::Esquerda, true);
        s.posicao_alternar(1);
        let linha = &s.layouts[0].linhas[0];
        let p = linha.pedacos.last().unwrap();
        assert!(p.tamanho < 8.0 && p.desloc > 2.0);
        s.posicao_alternar(1);
        assert_eq!(texto0(&s).len(), 1);
    }

    #[test]
    fn aumentar_fonte_anda_de_1_ate_12_e_de_2_depois() {
        let mut s = doc(&["abc"]);
        s.selecionar_tudo();
        s.tamanho_definir(11.0).unwrap();
        s.tamanho_passo(1);
        assert_eq!(s.formato_do_cursor().tamanho, 12.0);
        s.tamanho_passo(1);
        assert_eq!(s.formato_do_cursor().tamanho, 14.0);
        s.tamanho_passo(-1);
        s.tamanho_passo(-1);
        assert_eq!(s.formato_do_cursor().tamanho, 11.0);
    }

    #[test]
    fn limpar_formatacao_tira_tudo_menos_o_link() {
        let mut s = doc(&["**negrito** [site](http://a.b)"]);
        s.selecionar_tudo();
        s.cor_definir("#00ff00").unwrap();
        s.limpar_formatacao();
        let t = texto0(&s);
        assert!(t.iter().all(|x| x.estilo == 0 && x.fmt.eh_padrao()));
        assert!(t.iter().any(|x| x.link.is_some()));
    }

    #[test]
    fn maiusculas_nos_cinco_modos() {
        let mut s = doc(&["olá mundo. segunda frase"]);
        s.selecionar_tudo();
        s.maiusculas("maiusculas").unwrap();
        assert_eq!(crate::texto::plano(&texto0(&s)), "OLÁ MUNDO. SEGUNDA FRASE");
        s.maiusculas("minusculas").unwrap();
        s.maiusculas("palavras").unwrap();
        assert_eq!(crate::texto::plano(&texto0(&s)), "Olá Mundo. Segunda Frase");
        s.maiusculas("minusculas").unwrap();
        s.maiusculas("frase").unwrap();
        assert_eq!(crate::texto::plano(&texto0(&s)), "Olá mundo. Segunda frase");
        s.maiusculas("alternar").unwrap();
        assert_eq!(crate::texto::plano(&texto0(&s)), "oLÁ MUNDO. sEGUNDA FRASE");
        assert!(s.maiusculas("xis").is_err());
    }

    #[test]
    fn pincel_leva_a_formatacao_para_outra_selecao() {
        let mut s = doc(&["abc def"]);
        selecionar(&mut s, 3);
        s.formatar(crate::modelo::NEGRITO);
        s.cor_definir("#0000ff").unwrap();
        s.alinhar("direita").unwrap();
        s.mover(Movimento::InicioDoDocumento, false);
        s.mover(Movimento::Direita, false);
        s.pincel_copiar();
        s.mover(Movimento::FimDoDocumento, false);
        s.mover(Movimento::Esquerda, true);
        s.mover(Movimento::Esquerda, true);
        s.pincel_aplicar();
        let t = texto0(&s);
        assert_eq!(t.last().unwrap().fmt.cor, 0x0000_ffff);
        assert!(t.last().unwrap().estilo & crate::modelo::NEGRITO != 0);
    }
}
