//! Ortografia: as palavras que o dicionário (Hunspell, pelo `spellbook`) não conhece, nas páginas à
//! vista, para o sublinhado vermelho; e as sugestões da palavra do cursor. Tudo local.
//!
//! O dicionário é um só para o aplicativo (ele leva um ou dois segundos para carregar): o app o
//! instala uma vez com [`instalar`], e toda `Sessao` passa a usá-lo.
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use crate::alvo::{texto_alvo, Cel};
use crate::cursor::{Cursor, Pos};
use crate::geometria::Ret;
use crate::modelo::Conteudo;
use crate::sessao::{Mudanca, Sessao};
use crate::texto::plano;

/// O dicionário carregado.
pub struct Corretor {
    dic: spellbook::Dictionary,
}

impl Corretor {
    /// Lê os arquivos `.aff` e `.dic` do Hunspell (UTF-8, com ou sem BOM).
    pub fn novo(aff: &str, dic: &str) -> Result<Self, String> {
        let sem_bom = |s: &str| s.strip_prefix('\u{feff}').unwrap_or(s).to_string();
        spellbook::Dictionary::new(&sem_bom(aff), &sem_bom(dic)).map(|dic| Self { dic }).map_err(|e| format!("Dicionário inválido: {e}"))
    }

    pub fn correta(&self, palavra: &str) -> bool {
        self.dic.check(palavra)
    }

    /// Até cinco sugestões, a mais provável primeiro.
    pub fn sugestoes(&self, palavra: &str) -> Vec<String> {
        let mut v = Vec::new();
        self.dic.suggest(palavra, &mut v);
        v.truncate(5);
        v
    }
}

#[derive(Default)]
struct Estado {
    corretor: Option<Arc<Corretor>>,
    ligado: bool,
    /// As palavras que a pessoa mandou aceitar ("Adicionar ao dicionário"), em minúsculas.
    aceitas: HashSet<String>,
}

static ESTADO: RwLock<Option<Estado>> = RwLock::new(None);

fn com_estado<R>(f: impl FnOnce(&mut Estado) -> R) -> R {
    let mut e = ESTADO.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(e.get_or_insert_with(|| Estado { ligado: true, ..Estado::default() }))
}

/// Passa a verificar com este dicionário.
pub fn instalar(c: Corretor) {
    com_estado(|e| e.corretor = Some(Arc::new(c)));
}

/// Liga ou desliga o sublinhado (o dicionário continua carregado).
pub fn ligar(sim: bool) {
    com_estado(|e| e.ligado = sim);
}

/// Aceita a palavra daqui em diante (o dicionário pessoal; quem guarda a lista é o app).
pub fn aceitar(palavra: &str) {
    com_estado(|e| e.aceitas.insert(palavra.to_lowercase()));
}

/// O corretor e as palavras aceitas, se a verificação está ligada e o dicionário carregado.
fn ativo() -> Option<(Arc<Corretor>, HashSet<String>)> {
    com_estado(|e| e.corretor.clone().filter(|_| e.ligado).map(|c| (c, e.aceitas.clone())))
}

/// As palavras de `texto` que valem verificar, em posições de caractere `[a, b)`. Ficam de fora a
/// sigla (só maiúsculas), a letra solta e o que é endereço (`@`, `://`, `www.`).
pub fn palavras(texto: &str) -> Vec<(usize, usize, String)> {
    let cs: Vec<char> = texto.chars().collect();
    let letra = |k: usize| cs.get(k).is_some_and(|c| c.is_alphabetic());
    let mut saida = Vec::new();
    let mut k = 0;
    while k < cs.len() {
        if !letra(k) {
            k += 1;
            continue;
        }
        let a = k;
        // O hífen e o apóstrofo entre letras ficam dentro da palavra (guarda-chuva, d'água).
        while letra(k) || (matches!(cs.get(k), Some('-' | '\'' | '’')) && letra(k + 1) && k > a) {
            k += 1;
        }
        let palavra: String = cs[a..k].iter().collect();
        // A palavra colada em número (H2O, 3D) e o pedaço de endereço não são verificados.
        let inicio = cs[..a].iter().rposition(|c| c.is_whitespace()).map_or(0, |p| p + 1);
        let fim = cs[k..].iter().position(|c| c.is_whitespace()).map_or(cs.len(), |p| k + p);
        let token: String = cs[inicio..fim].iter().collect();
        let endereco = token.contains('@') || token.contains("://") || token.starts_with("www.");
        let colada = cs.get(k).is_some_and(char::is_ascii_digit) || a > 0 && cs[a - 1].is_ascii_digit();
        let sigla = palavra.chars().all(|c| !c.is_lowercase());
        if palavra.chars().count() > 1 && !endereco && !colada && !sigla {
            saida.push((a, k, palavra));
        }
    }
    saida
}

/// As palavras erradas de `texto`, em posições de caractere.
fn erros(c: &Corretor, aceitas: &HashSet<String>, texto: &str) -> Vec<(usize, usize, String)> {
    palavras(texto).into_iter().filter(|(_, _, p)| !c.correta(p) && !aceitas.contains(&p.to_lowercase())).collect()
}

/// As palavras erradas de `texto`; `None` com a verificação desligada ou sem dicionário.
pub(crate) fn erros_do_texto(texto: &str) -> Option<Vec<(usize, usize, String)>> {
    ativo().map(|(c, aceitas)| erros(&c, &aceitas, texto))
}

/// O que se verifica num bloco: o texto corrido e as células; o código e a fórmula, não.
fn alvos(c: &Conteudo) -> Vec<Cel> {
    if matches!(c, Conteudo::Codigo { .. }) {
        Vec::new()
    } else {
        crate::busca::alvos_do_bloco(c)
    }
}

impl Sessao {
    /// O sublinhado das palavras erradas nas páginas `de..=ate`.
    // shortcut: verifica as palavras de novo a cada pedido (as páginas à vista, alguns milhares de palavras); guardar por bloco se pesar.
    pub fn retangulos_da_ortografia(&self, de: usize, ate: usize) -> Vec<Ret> {
        let Some((c, aceitas)) = ativo() else { return Vec::new() };
        let mut blocos: Vec<usize> = self.paginas.iter().skip(de).take(ate + 1 - de.min(ate + 1)).flat_map(|p| p.fatias.iter().map(|f| f.bloco)).collect();
        blocos.sort_unstable();
        blocos.dedup();
        let mut saida = Vec::new();
        for i in blocos {
            let b = &self.doc.blocos[i];
            for celula in alvos(&b.conteudo) {
                let Some(t) = texto_alvo(&b.conteudo, celula) else { continue };
                for (a, z, _) in erros(&c, &aceitas, &plano(&t)) {
                    let pos = |desl| Some(Pos { bloco: b.id, celula, desl });
                    self.retangulos_do_bloco(i, (pos(a), pos(z)), false, de..=ate, &mut saida);
                }
            }
        }
        saida
    }

    /// A palavra errada sob o cursor (o começo, o fim e o texto) e as sugestões para ela.
    pub fn ortografia_no_cursor(&self) -> Option<(String, Vec<String>)> {
        let (c, aceitas) = ativo()?;
        let (_, _, palavra) = self.palavra_errada_no_cursor(&c, &aceitas)?;
        let sugestoes = c.sugestoes(&palavra);
        Some((palavra, sugestoes))
    }

    fn palavra_errada_no_cursor(&self, c: &Corretor, aceitas: &HashSet<String>) -> Option<(Pos, usize, String)> {
        let foco = self.cursor.foco;
        let b = &self.doc.blocos[self.idx(foco.bloco)?];
        let t = plano(&texto_alvo(&b.conteudo, foco.celula)?);
        let (a, z, p) = erros(c, aceitas, &t).into_iter().find(|(a, z, _)| (*a..=*z).contains(&foco.desl))?;
        Some((Pos { desl: a, ..foco }, z, p))
    }

    /// Troca a palavra errada do cursor pela sugestão `nova` (um Ctrl+Z).
    pub fn ortografia_trocar(&mut self, nova: &str) -> Mudanca {
        let Some((c, aceitas)) = ativo() else { return self.sem_mudanca() };
        let Some((inicio, fim, _)) = self.palavra_errada_no_cursor(&c, &aceitas) else { return self.sem_mudanca() };
        let m = self.trocar_em_bloco(inicio.bloco, &[(inicio.celula, inicio.desl, fim, nova.to_string())], Some(inicio.desl + nova.chars().count()));
        self.cursor = Cursor::em(Pos { desl: inicio.desl + nova.chars().count(), ..inicio });
        m
    }
}

impl Sessao {
    /// A palavra sob o cursor (qualquer uma, certa ou errada): o começo, o fim e o texto.
    fn palavra_no_cursor_em(&self) -> Option<(Pos, usize, String)> {
        let foco = self.cursor.foco;
        let b = &self.doc.blocos[self.idx(foco.bloco)?];
        if matches!(b.conteudo, Conteudo::Codigo { .. }) {
            return None;
        }
        let t = plano(&texto_alvo(&b.conteudo, foco.celula)?);
        let (a, z, p) = palavras(&t).into_iter().find(|(a, z, _)| (*a..=*z).contains(&foco.desl))?;
        Some((Pos { desl: a, ..foco }, z, p))
    }

    /// A palavra sob o cursor, para os sinônimos do botão direito.
    pub fn palavra_no_cursor(&self) -> Option<String> {
        self.palavra_no_cursor_em().map(|(_, _, p)| p)
    }

    /// Troca a palavra do cursor por `nova` (um sinônimo), com a maiúscula inicial dela (um Ctrl+Z).
    pub fn trocar_palavra_no_cursor(&mut self, nova: &str) -> Mudanca {
        let Some((inicio, fim, velha)) = self.palavra_no_cursor_em() else { return self.sem_mudanca() };
        let mut nova = nova.to_string();
        if velha.chars().next().is_some_and(char::is_uppercase) {
            let mut cs = nova.chars();
            nova = cs.next().map(|c| c.to_uppercase().chain(cs).collect()).unwrap_or_default();
        }
        let n = nova.chars().count();
        let m = self.trocar_em_bloco(inicio.bloco, &[(inicio.celula, inicio.desl, fim, nova)], Some(inicio.desl + n));
        self.cursor = Cursor::em(Pos { desl: inicio.desl + n, ..inicio });
        m
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::{Documento, Trecho};

    const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwjqxz\n";
    const DIC: &str = "\u{feff}5\ncasa\nazul\nexceção\nguarda-chuva\numa\n";

    #[test]
    fn troca_a_palavra_do_cursor_mantendo_a_maiuscula() {
        let mut d = Documento::vazio();
        d.blocos = vec![crate::modelo::Bloco::novo(1, Conteudo::Paragrafo { texto: vec![Trecho::simples("Bonito dia")] })];
        let mut s = Sessao::com_documento(d);
        let id = s.documento().blocos[0].id;
        s.cursor = Cursor::em(Pos { bloco: id, celula: None, desl: 2 });
        assert_eq!(s.palavra_no_cursor().as_deref(), Some("Bonito"));
        s.trocar_palavra_no_cursor("belo");
        assert_eq!(crate::inline::texto_puro(crate::alvo::texto_alvo(&s.documento().blocos[0].conteudo, None).unwrap().as_slice()), "Belo dia");
    }

    #[test]
    fn separa_as_palavras_que_valem_verificar() {
        let p: Vec<String> = palavras("A casa (azul) d'água, guarda-chuva - ONU, H2O, ana@x.com e www.site.br; x.").into_iter().map(|x| x.2).collect();
        assert_eq!(p, ["casa", "azul", "d'água", "guarda-chuva"]);
        assert_eq!(palavras("ação é")[0], (0, 4, "ação".into()), "posições em caracteres");
    }

    #[test]
    fn acha_as_erradas_sugere_aceita_e_troca() {
        let c = Corretor::novo(AFF, DIC).unwrap();
        assert!(c.correta("exceção") && !c.correta("excessão"));
        assert!(c.sugestoes("cassa").contains(&"casa".to_string()));
        let errado: Vec<String> = erros(&c, &HashSet::new(), "A cassa azul e a excessão.").into_iter().map(|x| x.2).collect();
        assert_eq!(errado, ["cassa", "excessão"]);
        assert!(erros(&c, &["cassa".to_string()].into(), "Cassa").is_empty(), "a aceita não sublinha, nem em maiúscula");

        instalar(c);
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("Uma cassa azul.")] });
        d.acrescentar(Conteudo::Codigo { linguagem: String::new(), codigo: "cassa".into() });
        let mut s = Sessao::com_documento(d);
        assert_eq!(s.retangulos_da_ortografia(0, 0).len(), 1, "o código não é verificado");
        s.cursor = Cursor::em(Pos { bloco: s.documento().blocos[0].id, celula: None, desl: 6 });
        let (palavra, sugestoes) = s.ortografia_no_cursor().unwrap();
        assert_eq!((palavra.as_str(), sugestoes.contains(&"casa".to_string())), ("cassa", true));
        s.ortografia_trocar("casa");
        assert_eq!(plano(s.documento().blocos[0].conteudo.textos()[0]), "Uma casa azul.");
        assert!(s.retangulos_da_ortografia(0, 0).is_empty());
        s.desfazer();
        ligar(false);
        assert!(s.retangulos_da_ortografia(0, 0).is_empty(), "desligado, nada sublinha");
        ligar(true);
    }
}
