//! Localizar e substituir: diferenciar maiúsculas, palavra inteira e expressão regular.
//! Os achados valem para o texto de todos os blocos (e de cada célula de tabela); o que
//! está selecionado é o achado atual. Substituir mantém a formatação do primeiro caractere trocado.

use regex::{Regex, RegexBuilder};
use serde::Serialize;

use crate::alvo::{com_texto_ref, dimensoes, eh_atomico, texto_alvo, Cel};
use crate::cursor::{Cursor, Pos};
use crate::geometria::Ret;
use crate::modelo::Conteudo;
use crate::sessao::{Mudanca, Sessao};
use crate::texto::{apagar, fatia, inserir, marca_em, plano};

/// As opções do painel de busca.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct Opcoes {
    #[serde(default)]
    pub maiusculas: bool,
    #[serde(default)]
    pub palavra: bool,
    #[serde(default)]
    pub regex: bool,
}

/// Um trecho achado: o bloco, a célula e os caracteres `[a, b)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Achado {
    pub bloco: u64,
    pub celula: Cel,
    pub a: usize,
    pub b: usize,
}

/// A busca ativa.
pub struct Busca {
    re: Regex,
    opcoes: Opcoes,
}

/// O que o painel mostra depois de cada pedido: "3 de 12" (ou o erro da expressão).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Resultado {
    pub total: usize,
    /// A posição (de 1) do achado selecionado; 0 se nenhum.
    pub atual: usize,
    pub erro: Option<String>,
}

fn compilar(consulta: &str, o: Opcoes) -> Result<Regex, String> {
    let corpo = if o.regex { consulta.to_string() } else { regex::escape(consulta) };
    let corpo = if o.palavra { format!(r"\b(?:{corpo})\b") } else { corpo };
    RegexBuilder::new(&corpo).case_insensitive(!o.maiusculas).build().map_err(|e| format!("Expressão inválida: {}", e.to_string().lines().last().unwrap_or("")))
}

/// Os achados de `re` em `texto`, em posições de caractere (achados vazios não contam). O achado
/// que passa por cima de uma nota, comentário ou campo (o `OBJETO` no texto) também não: trocá-lo
/// apagaria o objeto junto (o `.` da expressão regular casa com ele).
fn achar(re: &Regex, texto: &str) -> Vec<(usize, usize)> {
    let inicios: Vec<usize> = texto.char_indices().map(|(i, _)| i).collect();
    let char_de = |byte: usize| inicios.partition_point(|&i| i < byte);
    re.find_iter(texto).filter(|m| !m.is_empty() && !m.as_str().contains(crate::modelo::OBJETO)).map(|m| (char_de(m.start()), char_de(m.end()))).collect()
}

/// As células (ou o bloco inteiro) que têm texto procurável.
pub(crate) fn alvos_do_bloco(c: &Conteudo) -> Vec<Cel> {
    if let Some((linhas, colunas)) = dimensoes(c) {
        return (0..linhas).flat_map(|l| (0..colunas).map(move |k| Some((l as u32, k as u32)))).collect();
    }
    if eh_atomico(c) || matches!(c, Conteudo::Equacao { .. }) {
        Vec::new()
    } else {
        vec![None]
    }
}

impl Sessao {
    fn achados_do_bloco(&self, i: usize, re: &Regex) -> Vec<Achado> {
        let b = &self.doc.blocos[i];
        let mut saida = Vec::new();
        for celula in alvos_do_bloco(&b.conteudo) {
            let Some(t) = texto_alvo(&b.conteudo, celula) else { continue };
            saida.extend(achar(re, &plano(&t)).into_iter().map(|(a, z)| Achado { bloco: b.id, celula, a, b: z }));
        }
        saida
    }

    fn todos_os_achados(&self, re: &Regex) -> Vec<Achado> {
        (0..self.doc.blocos.len()).flat_map(|i| self.achados_do_bloco(i, re)).collect()
    }

    fn selecionar_achado(&mut self, x: Achado) {
        let pos = |desl| Pos { bloco: x.bloco, celula: x.celula, desl };
        self.cursor = Cursor { ancora: pos(x.a), foco: pos(x.b), x_desejado: None, pendente: None };
        self.digitando = None;
    }

    /// Qual achado é a seleção de agora (se for um).
    fn indice_do_selecionado(&self, todos: &[Achado]) -> Option<usize> {
        let (ini, fim) = self.selecao_ordenada();
        todos.iter().position(|x| ini.bloco == x.bloco && fim.bloco == x.bloco && ini.celula == x.celula && ini.desl == x.a && fim.desl == x.b)
    }

    fn resultado(&self, todos: &[Achado]) -> Resultado {
        Resultado { total: todos.len(), atual: self.indice_do_selecionado(todos).map_or(0, |k| k + 1), erro: None }
    }

    /// Começa (ou muda) a busca: seleciona o primeiro achado a partir do cursor. Consulta vazia encerra a busca.
    pub fn busca_definir(&mut self, consulta: &str, opcoes: Opcoes) -> Resultado {
        if consulta.is_empty() {
            self.busca = None;
            return Resultado::default();
        }
        let re = match compilar(consulta, opcoes) {
            Ok(re) => re,
            Err(erro) => {
                self.busca = None;
                return Resultado { erro: Some(erro), ..Resultado::default() };
            }
        };
        let todos = self.todos_os_achados(&re);
        self.busca = Some(Busca { re, opcoes });
        let (ini, _) = self.selecao_ordenada();
        let chave = |x: &Achado| self.chave(&Pos { bloco: x.bloco, celula: x.celula, desl: x.a });
        let primeiro = todos.iter().find(|x| chave(x) >= self.chave(&ini)).or(todos.first()).copied();
        if let Some(x) = primeiro {
            self.selecionar_achado(x);
        }
        self.resultado(&todos)
    }

    /// Vai ao achado seguinte (`+1`) ou anterior (`-1`), dando a volta no documento.
    pub fn busca_ir(&mut self, delta: i32) -> Resultado {
        let Some(re) = self.busca.as_ref().map(|b| b.re.clone()) else { return Resultado::default() };
        let todos = self.todos_os_achados(&re);
        if todos.is_empty() {
            return Resultado::default();
        }
        let n = todos.len() as i64;
        let k = match self.indice_do_selecionado(&todos) {
            Some(k) => (k as i64 + i64::from(delta.signum())).rem_euclid(n) as usize,
            None => self.proximo_a_partir_do_cursor(&todos, delta),
        };
        self.selecionar_achado(todos[k]);
        self.resultado(&todos)
    }

    fn proximo_a_partir_do_cursor(&self, todos: &[Achado], delta: i32) -> usize {
        let (ini, fim) = self.selecao_ordenada();
        let chave = |x: &Achado| self.chave(&Pos { bloco: x.bloco, celula: x.celula, desl: x.a });
        if delta >= 0 {
            todos.iter().position(|x| chave(x) >= self.chave(&fim)).unwrap_or(0)
        } else {
            todos.iter().rposition(|x| chave(x) < self.chave(&ini)).unwrap_or(todos.len() - 1)
        }
    }

    /// Encerra a busca (tira o realce dos achados).
    pub fn busca_limpar(&mut self) {
        self.busca = None;
    }

    /// O que `por` vira para o achado `x`: com expressão regular, `$1` e companhia valem.
    fn texto_novo(&self, x: &Achado, por: &str) -> String {
        let Some(b) = &self.busca else { return por.to_string() };
        if !b.opcoes.regex {
            return por.to_string();
        }
        let Some(t) = self.idx(x.bloco).and_then(|i| texto_alvo(&self.doc.blocos[i].conteudo, x.celula)) else { return por.to_string() };
        let texto = plano(&t);
        let byte = texto.char_indices().nth(x.a).map_or(texto.len(), |(i, _)| i);
        let mut saida = String::new();
        match b.re.captures_at(&texto, byte) {
            Some(c) => c.expand(por, &mut saida),
            None => saida.push_str(por),
        }
        saida
    }

    /// Troca o achado selecionado por `por` e vai ao próximo; se a seleção não é um achado, só vai ao próximo.
    pub fn substituir(&mut self, por: &str) -> (Mudanca, Resultado) {
        let Some(re) = self.busca.as_ref().map(|b| b.re.clone()) else { return (self.sem_mudanca(), Resultado::default()) };
        let todos = self.todos_os_achados(&re);
        let Some(k) = self.indice_do_selecionado(&todos) else {
            let r = self.busca_ir(1);
            return (self.sem_mudanca(), r);
        };
        let x = todos[k];
        let novo = self.texto_novo(&x, por);
        let mudanca = self.trocar_em_bloco(x.bloco, &[(x.celula, x.a, x.b, novo.clone())], Some(x.a + novo.chars().count()));
        let r = self.busca_ir(1);
        (mudanca, r)
    }

    /// Troca todos os achados; devolve a mudança e quantos foram.
    pub fn substituir_tudo(&mut self, por: &str) -> (Mudanca, usize) {
        let Some(re) = self.busca.as_ref().map(|b| b.re.clone()) else { return (self.sem_mudanca(), 0) };
        let todos = self.todos_os_achados(&re);
        if todos.is_empty() {
            return (self.sem_mudanca(), 0);
        }
        let trocas: Vec<(Achado, String)> = todos.iter().map(|x| (*x, self.texto_novo(x, por))).collect();
        let (i0, i1) = (self.idx(todos[0].bloco).unwrap_or(0), self.idx(todos[todos.len() - 1].bloco).unwrap_or(0));
        let mut t = self.trabalho(i0, i1);
        for (x, novo) in trocas.iter().rev() {
            if let Some(b) = t.blocos.iter_mut().find(|b| b.id == x.bloco) {
                com_texto_ref(&mut b.conteudo, x.celula, |texto| trocar(texto, x.a, x.b, novo));
            }
        }
        let cursor = Cursor::em(self.inicio_do_bloco(i0));
        (self.fechar_trabalho(t, cursor, None), todos.len())
    }

    pub(crate) fn trocar_em_bloco(&mut self, id: u64, trocas: &[(Cel, usize, usize, String)], cursor_em: Option<usize>) -> Mudanca {
        let Some(i) = self.idx(id) else { return self.sem_mudanca() };
        let mut t = self.trabalho(i, i);
        for (celula, a, b, novo) in trocas.iter().rev() {
            com_texto_ref(&mut t.blocos[0].conteudo, *celula, |texto| trocar(texto, *a, *b, novo));
        }
        let celula = trocas.first().and_then(|x| x.0);
        let cursor = Cursor::em(Pos { bloco: id, celula, desl: cursor_em.unwrap_or(0) });
        self.fechar_trabalho(t, cursor, None)
    }

    /// Os retângulos de todos os achados nas páginas `de..=ate` (o realce amarelo do painel).
    pub fn retangulos_dos_achados(&self, de: usize, ate: usize) -> Vec<Ret> {
        let Some(b) = &self.busca else { return Vec::new() };
        let mut blocos: Vec<usize> = self.paginas.iter().skip(de).take(ate + 1 - de.min(ate + 1)).flat_map(|p| p.fatias.iter().map(|f| f.bloco)).collect();
        blocos.sort_unstable();
        blocos.dedup();
        let mut saida = Vec::new();
        for i in blocos {
            for x in self.achados_do_bloco(i, &b.re) {
                let pos = |desl| Some(Pos { bloco: x.bloco, celula: x.celula, desl });
                self.retangulos_do_bloco(i, (pos(x.a), pos(x.b)), false, de..=ate, &mut saida);
            }
        }
        saida
    }
}

/// Troca o texto `[a, b)` por `novo`, com a formatação do primeiro caractere trocado.
fn trocar(t: &mut Vec<crate::modelo::Trecho>, a: usize, b: usize, novo: &str) {
    let (estilo, fmt) = if fatia(t, a, a + 1).is_empty() { marca_em(t, a) } else { marca_em(t, a + 1) };
    apagar(t, a, b);
    inserir(t, a, novo, estilo, fmt);
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::inline::analisar;
    use crate::modelo::Documento;

    fn doc(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        Sessao::com_documento(d)
    }

    fn tudo(s: &Sessao) -> Vec<String> {
        s.documento().blocos.iter().map(|b| texto_alvo(&b.conteudo, None).map(|t| plano(&t)).unwrap_or_default()).collect()
    }

    #[test]
    fn achado_nao_engole_o_objeto_preso_ao_texto() {
        let re = compilar("a.b", Opcoes { regex: true, ..Opcoes::default() }).unwrap();
        assert!(achar(&re, &format!("a{}b", crate::modelo::OBJETO)).is_empty());
        assert_eq!(achar(&re, "axb"), [(0, 3)]);
    }

    #[test]
    fn conta_navega_e_da_a_volta() {
        let mut s = doc(&["Gato e gato", "outro GATO aqui"]);
        let r = s.busca_definir("gato", Opcoes::default());
        assert_eq!((r.total, r.atual), (3, 1));
        assert_eq!(s.busca_ir(1).atual, 2);
        assert_eq!(s.busca_ir(1).atual, 3);
        assert_eq!(s.busca_ir(1).atual, 1);
        assert_eq!(s.busca_ir(-1).atual, 3);
        let r = s.busca_definir("gato", Opcoes { maiusculas: true, ..Opcoes::default() });
        assert_eq!(r.total, 1);
        assert!(!s.retangulos_dos_achados(0, 0).is_empty());
    }

    #[test]
    fn palavra_inteira_e_regex() {
        let mut s = doc(&["gato gatos agato"]);
        assert_eq!(s.busca_definir("gato", Opcoes { palavra: true, ..Opcoes::default() }).total, 1);
        assert_eq!(s.busca_definir("gato", Opcoes::default()).total, 3);
        assert_eq!(s.busca_definir(r"g\w+o", Opcoes { regex: true, ..Opcoes::default() }).total, 3);
        let r = s.busca_definir("(", Opcoes { regex: true, ..Opcoes::default() });
        assert!(r.erro.is_some());
        assert_eq!(s.busca_definir("a.b", Opcoes::default()).total, 0, "sem regex o ponto é só ponto");
    }

    #[test]
    fn substituir_um_e_todos_mantem_a_formatacao() {
        let mut s = doc(&["um **gato** e um gato"]);
        s.busca_definir("gato", Opcoes::default());
        let (m, r) = s.substituir("cão");
        assert!(!m.afetados.is_empty());
        assert_eq!(tudo(&s)[0], "um cão e um gato");
        assert_eq!((r.total, r.atual), (1, 1));
        let Conteudo::Paragrafo { texto } = &s.documento().blocos[0].conteudo else { panic!() };
        assert!(texto.iter().any(|t| t.texto.contains("cão") && t.estilo & crate::modelo::NEGRITO != 0));
        let (_, n) = s.substituir_tudo("rato");
        assert_eq!(n, 1);
        assert_eq!(tudo(&s)[0], "um cão e um rato");
        s.desfazer();
        assert_eq!(tudo(&s)[0], "um cão e um gato");
    }

    #[test]
    fn regex_substitui_com_grupos() {
        let mut s = doc(&["2026-10-06 e 2025-01-02"]);
        s.busca_definir(r"(\d+)-(\d+)-(\d+)", Opcoes { regex: true, ..Opcoes::default() });
        let (_, n) = s.substituir_tudo("$3/$2/$1");
        assert_eq!(n, 2);
        assert_eq!(tudo(&s)[0], "06/10/2026 e 02/01/2025");
    }
}
