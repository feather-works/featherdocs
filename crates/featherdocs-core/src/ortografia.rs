//! Ortografia: as palavras que o dicionário (Hunspell, pelo `spellbook`) não conhece, nas páginas à
//! vista, para o sublinhado vermelho; e as sugestões da palavra do cursor. Tudo local.
//!
//! Os dicionários são do aplicativo, um por língua (levam um ou dois segundos para carregar): o app
//! os instala uma vez com [`instalar_idioma`], e toda `Sessao` passa a usá-los. Cada trecho é
//! verificado no dicionário da língua dele (Revisão → Idioma, o `w:lang` do Word); sem língua, a do
//! documento, e sem ela a do aplicativo ([`idioma_do_app`]). Língua sem dicionário não é verificada.
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use crate::alvo::{texto_alvo, Cel};
use crate::cursor::{Cursor, Pos};
use crate::geometria::Ret;
use crate::modelo::{Conteudo, Familia, Trecho};
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
    /// Os dicionários pelo prefixo da língua (`pt`, `en`, `es`).
    corretores: Vec<(String, Arc<Corretor>)>,
    /// A língua do texto que não diz a sua, nem o documento (a da interface; vazio = português).
    padrao: String,
    ligado: bool,
    /// As palavras que a pessoa mandou aceitar ("Adicionar ao dicionário"), em minúsculas.
    aceitas: HashSet<String>,
}

static ESTADO: RwLock<Option<Estado>> = RwLock::new(None);

fn com_estado<R>(f: impl FnOnce(&mut Estado) -> R) -> R {
    let mut e = ESTADO.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(e.get_or_insert_with(|| Estado { ligado: true, ..Estado::default() }))
}

/// Passa a verificar o português com este dicionário.
pub fn instalar(c: Corretor) {
    instalar_idioma("pt", c);
}

/// Passa a verificar a língua `idioma` (`en`, `es-ES`: vale o prefixo) com este dicionário.
pub fn instalar_idioma(idioma: &str, c: Corretor) {
    let prefixo = prefixo(idioma);
    com_estado(|e| {
        e.corretores.retain(|(p, _)| *p != prefixo);
        e.corretores.push((prefixo, Arc::new(c)));
    });
}

/// A língua do texto que não diz a sua (a da interface do app, `es-ES`).
pub fn idioma_do_app(idioma: &str) {
    com_estado(|e| e.padrao = idioma.to_string());
}

/// O prefixo de duas letras, em minúsculas (`pt-BR` → `pt`); vazio é português.
fn prefixo(idioma: &str) -> String {
    let p = idioma.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    if p.is_empty() { "pt".into() } else { p }
}

/// Liga ou desliga o sublinhado (o dicionário continua carregado).
pub fn ligar(sim: bool) {
    com_estado(|e| e.ligado = sim);
}

/// Aceita a palavra daqui em diante (o dicionário pessoal; quem guarda a lista é o app).
pub fn aceitar(palavra: &str) {
    com_estado(|e| e.aceitas.insert(palavra.to_lowercase()));
}

/// O que a verificação usa agora: os dicionários, as palavras aceitas e a língua do app.
struct Ativo {
    corretores: Vec<(String, Arc<Corretor>)>,
    aceitas: HashSet<String>,
    padrao: String,
}

impl Ativo {
    /// O dicionário de `idioma` (vazio: o `documento`, e sem ele a língua do app).
    fn de(&self, idioma: &str, documento: &str) -> Option<&Arc<Corretor>> {
        let idioma = [idioma, documento, &self.padrao].into_iter().find(|i| !i.is_empty()).unwrap_or("");
        let p = prefixo(idioma);
        self.corretores.iter().find(|(q, _)| *q == p).map(|(_, c)| c)
    }
}

/// O que a verificação usa, se ela está ligada e há ao menos um dicionário carregado.
fn ativo() -> Option<Ativo> {
    com_estado(|e| (e.ligado && !e.corretores.is_empty()).then(|| Ativo { corretores: e.corretores.clone(), aceitas: e.aceitas.clone(), padrao: e.padrao.clone() }))
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

/// As palavras erradas de `texto` e os deslizes de digitação ([`deslizes`]), num dicionário só (os testes).
#[cfg(test)]
fn erros(c: &Corretor, aceitas: &HashSet<String>, texto: &str) -> Vec<(usize, usize, String)> {
    let mut v: Vec<_> = palavras(texto).into_iter().filter(|(_, _, p)| !c.correta(p)).collect();
    v.extend(deslizes(texto));
    v.retain(|(_, _, p)| !aceitas.contains(&p.to_lowercase()));
    v.sort_by_key(|x| x.0);
    v
}

/// Deslizes que o dicionário não pega, porque as palavras existem: a palavra repetida ("de de"),
/// o espaço duplo entre palavras e o espaço antes de vírgula, ponto e afins. O texto achado tem
/// sempre um espaço (uma palavra errada nunca tem): é assim que [`correcao_do_deslize`] o reconhece.
pub fn deslizes(texto: &str) -> Vec<(usize, usize, String)> {
    let cs: Vec<char> = texto.chars().collect();
    let achado = |a: usize, z: usize| (a, z, cs[a..z].iter().collect::<String>());
    let mut v = Vec::new();
    // Palavras (letras seguidas) e onde estão.
    let mut ps: Vec<(usize, usize)> = Vec::new();
    let mut k = 0;
    while k < cs.len() {
        if cs[k].is_alphabetic() {
            let a = k;
            while k < cs.len() && cs[k].is_alphabetic() {
                k += 1;
            }
            ps.push((a, k));
        } else {
            k += 1;
        }
    }
    for w in ps.windows(2) {
        let ((a, b), (c, d)) = (w[0], w[1]);
        let so_espaco = cs[b..c].iter().all(|&x| x == ' ');
        let igual = cs[a..b].iter().flat_map(|x| x.to_lowercase()).eq(cs[c..d].iter().flat_map(|x| x.to_lowercase()));
        if b < c && so_espaco && igual && b - a > 1 {
            v.push(achado(a, d));
        }
    }
    let mut k = 0;
    while k < cs.len() {
        if cs[k] != ' ' || k == 0 || cs[k - 1].is_whitespace() {
            k += 1;
            continue;
        }
        let a = k;
        while k < cs.len() && cs[k] == ' ' {
            k += 1;
        }
        let depois = cs.get(k).copied();
        let pontuacao = depois.is_some_and(|p| ",.;:!?)".contains(p)) && cs.get(k + 1).map_or(true, |x| x.is_whitespace());
        if pontuacao && !(depois == Some('.') && cs.get(k + 1) == Some(&'.')) {
            v.push(achado(a, k + 1));
        } else if k - a > 1 && depois.is_some_and(|x| !x.is_whitespace()) {
            v.push(achado(a, k));
        }
    }
    v
}

/// O que entra no lugar de um deslize achado por [`deslizes`]; `None` se `achado` é palavra.
pub fn correcao_do_deslize(achado: &str) -> Option<String> {
    if !achado.contains(' ') {
        return None;
    }
    Some(if achado.trim().is_empty() {
        " ".to_string()
    } else if achado.starts_with(' ') {
        achado.trim_start().to_string()
    } else {
        achado.split(' ').next().unwrap_or(achado).to_string()
    })
}

/// As palavras erradas do texto `t` e os deslizes (posições de caractere do texto inteiro, marcas
/// inclusive), cada palavra no dicionário da língua do trecho dela, com esse dicionário (para as
/// sugestões). O texto numa língua sem dicionário, e o das marcas, não é verificado.
/// `documento` é o idioma do documento (`w:lang`).
fn erros_no_texto(a: &Ativo, t: &[Trecho], documento: &str) -> Vec<(usize, usize, String, Option<Arc<Corretor>>)> {
    let mut faixas: Vec<(std::ops::Range<usize>, Option<&Arc<Corretor>>)> = Vec::new();
    let mut k = 0;
    for x in t {
        let n = x.texto.chars().count();
        let c = if x.marca.is_some() { None } else { a.de(if x.fmt.mais.idioma.vazia() { "" } else { x.fmt.mais.idioma.nome() }, documento) };
        faixas.push((k..k + n, c));
        k += n;
    }
    let dicionario = |p: usize| faixas.iter().find(|(f, _)| f.contains(&p)).and_then(|(_, c)| *c);
    let texto = plano(t);
    let mut v: Vec<_> = palavras(&texto).into_iter().filter_map(|(i, z, p)| dicionario(i).filter(|c| !c.correta(&p)).map(|c| (i, z, p, Some(c.clone())))).collect();
    v.extend(deslizes(&texto).into_iter().filter(|(i, ..)| dicionario(*i).is_some()).map(|(i, z, p)| (i, z, p, None)));
    v.retain(|(_, _, p, _)| !a.aceitas.contains(&p.to_lowercase()));
    v.sort_by_key(|x| x.0);
    v
}

/// As palavras erradas do texto `t`; `None` com a verificação desligada ou sem dicionário.
pub(crate) fn erros_do_texto(t: &[Trecho], documento: &str) -> Option<Vec<(usize, usize, String)>> {
    ativo().map(|a| erros_no_texto(&a, t, documento).into_iter().map(|(i, z, p, _)| (i, z, p)).collect())
}

/// Os idiomas que a Revisão oferece para um trecho (o nome do `w:lang` e o rótulo).
pub const IDIOMAS: [(&str, &str); 6] = [
    ("pt-BR", "Português"),
    ("en-US", "Inglês"),
    ("es-ES", "Espanhol"),
    ("fr-FR", "Francês"),
    ("de-DE", "Alemão"),
    ("it-IT", "Italiano"),
];

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
        let Some(ativo) = ativo() else { return Vec::new() };
        let mut blocos: Vec<usize> = self.paginas.iter().skip(de).take(ate + 1 - de.min(ate + 1)).flat_map(|p| p.fatias.iter().map(|f| f.bloco)).collect();
        blocos.sort_unstable();
        blocos.dedup();
        let mut saida = Vec::new();
        for i in blocos {
            let b = &self.doc.blocos[i];
            for celula in alvos(&b.conteudo) {
                let Some(t) = texto_alvo(&b.conteudo, celula) else { continue };
                for (a, z, ..) in erros_no_texto(&ativo, &t, &self.doc.extras.documento.idioma) {
                    let pos = |desl| Some(Pos { bloco: b.id, celula, desl });
                    self.retangulos_do_bloco(i, (pos(a), pos(z)), false, de..=ate, &mut saida);
                }
            }
        }
        saida
    }

    /// A palavra errada sob o cursor (o começo, o fim e o texto) e as sugestões para ela.
    pub fn ortografia_no_cursor(&self) -> Option<(String, Vec<String>)> {
        let (_, _, palavra, c) = self.palavra_errada_no_cursor(&ativo()?)?;
        let sugestoes = match (correcao_do_deslize(&palavra), c) {
            (Some(s), _) => vec![s],
            (None, Some(c)) => c.sugestoes(&palavra),
            (None, None) => Vec::new(),
        };
        Some((palavra, sugestoes))
    }

    fn palavra_errada_no_cursor(&self, ativo: &Ativo) -> Option<(Pos, usize, String, Option<Arc<Corretor>>)> {
        let foco = self.cursor.foco;
        let b = &self.doc.blocos[self.idx(foco.bloco)?];
        let t = texto_alvo(&b.conteudo, foco.celula)?;
        let (a, z, p, c) = erros_no_texto(ativo, &t, &self.doc.extras.documento.idioma).into_iter().find(|(a, z, ..)| (*a..=*z).contains(&foco.desl))?;
        Some((Pos { desl: a, ..foco }, z, p, c))
    }

    /// Troca a palavra errada do cursor pela sugestão `nova` (um Ctrl+Z).
    pub fn ortografia_trocar(&mut self, nova: &str) -> Mudanca {
        let Some(ativo) = ativo() else { return self.sem_mudanca() };
        let Some((inicio, fim, ..)) = self.palavra_errada_no_cursor(&ativo) else { return self.sem_mudanca() };
        let m = self.trocar_em_bloco(inicio.bloco, &[(inicio.celula, inicio.desl, fim, nova.to_string())], Some(inicio.desl + nova.chars().count()));
        self.cursor = Cursor::em(Pos { desl: inicio.desl + nova.chars().count(), ..inicio });
        m
    }
}

impl Sessao {
    /// Marca o texto selecionado (ou o que vai ser digitado) como escrito em `idioma` (`en-US`); a
    /// ortografia em português pula o que está em outra língua. Vai e volta do `.docx` (`w:lang`).
    pub fn idioma_definir(&mut self, idioma: &str) -> Mudanca {
        // Português no documento em português: tira a marca em vez de repetir o idioma do documento.
        let igual_ao_documento = prefixo(idioma) == prefixo(&self.idioma_do_documento());
        let f = if igual_ao_documento { Familia::default() } else { Familia::de(idioma) };
        self.mudar_marca(|_, fm| fm.mais.idioma = f)
    }

    /// O idioma do texto no cursor (o do documento quando o trecho não diz).
    pub fn idioma_no_cursor(&self) -> String {
        let (_, f) = self.marca_da_selecao();
        if f.mais.idioma.vazia() { self.idioma_do_documento() } else { f.mais.idioma.nome().to_string() }
    }

    /// O idioma do texto que não diz o seu: o do documento, senão o do app, senão `pt-BR`.
    fn idioma_do_documento(&self) -> String {
        let doc = &self.doc.extras.documento.idioma;
        if !doc.is_empty() {
            return doc.clone();
        }
        let app = com_estado(|e| e.padrao.clone());
        if app.is_empty() { "pt-BR".into() } else { app }
    }

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
    fn trecho_em_outra_lingua_nao_e_verificado() {
        let c = Corretor::novo(AFF, DIC).unwrap();
        let mut ingles = Trecho::simples("house blue ");
        ingles.fmt.mais.idioma = Familia::de("en-US");
        let t = vec![Trecho::simples("A cassa "), ingles, Trecho::simples("azull")];
        let so_pt = Ativo { corretores: vec![("pt".into(), Arc::new(c))], aceitas: HashSet::new(), padrao: String::new() };
        let errado: Vec<String> = erros_no_texto(&so_pt, &t, "").into_iter().map(|x| x.2).collect();
        assert_eq!(errado, ["cassa", "azull"], "o trecho em inglês fica de fora");
        // O documento inteiro em inglês: só o trecho marcado em português é verificado.
        let mut pt = Trecho::simples("cassa");
        pt.fmt.mais.idioma = Familia::de("pt-BR");
        let errado: Vec<String> = erros_no_texto(&so_pt, &[Trecho::simples("house "), pt.clone()], "en-US").into_iter().map(|x| x.2).collect();
        assert_eq!(errado, ["cassa"]);
        // Com o dicionário de inglês, o trecho em inglês é verificado nele (e não no de português).
        let ingles = Corretor::novo(AFF, "2\nhouse\nblue\n").unwrap();
        let dois = Ativo { corretores: vec![so_pt.corretores[0].clone(), ("en".into(), Arc::new(ingles))], aceitas: HashSet::new(), padrao: String::new() };
        let errado: Vec<String> = erros_no_texto(&dois, &[Trecho::simples("house hause "), pt], "en-US").into_iter().map(|x| x.2).collect();
        assert_eq!(errado, ["hause", "cassa"]);
        // Sem idioma no trecho nem no documento, vale a língua do app.
        let app_es = Ativo { padrao: "es-ES".into(), ..dois };
        assert!(erros_no_texto(&app_es, &[Trecho::simples("cassa hause")], "").is_empty(), "espanhol sem dicionário não é verificado");
    }

    #[test]
    fn idioma_definir_marca_a_selecao_e_desfaz() {
        let mut d = Documento::vazio();
        d.blocos = vec![crate::modelo::Bloco::novo(1, Conteudo::Paragrafo { texto: vec![Trecho::simples("Abstract here")] })];
        let mut s = Sessao::com_documento(d);
        s.selecionar_tudo();
        s.idioma_definir("en-US");
        assert_eq!(s.idioma_no_cursor(), "en-US");
        s.idioma_definir("pt-BR");
        assert_eq!(s.idioma_no_cursor(), "pt-BR", "português no documento em português tira a marca");
        assert!(crate::alvo::texto_alvo(&s.documento().blocos[0].conteudo, None).unwrap().iter().all(|t| t.fmt.mais.idioma.vazia()));
        s.desfazer();
        assert_eq!(s.idioma_no_cursor(), "en-US");
    }

    #[test]
    fn deslizes_de_digitacao_e_a_correcao_de_cada_um() {
        let t = "Foi de de carro  hoje , e depois... voltou. A a casa. Ele disse :) ok";
        let v: Vec<String> = deslizes(t).into_iter().map(|x| x.2).collect();
        assert_eq!(v, ["de de", "  ", " ,"], "a letra solta repetida, o :) e o ... ficam");
        let fix: Vec<String> = v.iter().map(|d| correcao_do_deslize(d).unwrap()).collect();
        assert_eq!(fix, ["de", " ", ","]);
        assert_eq!(correcao_do_deslize("casa"), None);
        assert!(deslizes("  recuo no começo e fim  ").is_empty());
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
