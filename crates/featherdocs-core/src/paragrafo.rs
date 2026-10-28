//! Atributos de parágrafo além do alinhamento e do recuo: entrelinha, espaço antes/depois,
//! sombreamento, bordas, o estilo das listas (marcadores, numeração, tarefas).

use crate::edicao::converter;
use crate::fonte::ler_cor;
use crate::digitacao::tem_texto_corrido;
use crate::modelo::{Atrib, Conteudo, Lista};
use crate::sessao::{Mudanca, Sessao};

/// O estilo de lista que `lista` pede e se é numerada.
fn lista_de(nome: &str) -> Result<(Lista, bool), String> {
    Ok(match nome {
        "padrao" => (Lista::Padrao, false),
        "circulo" => (Lista::Circulo, false),
        "quadrado" => (Lista::Quadrado, false),
        "seta" => (Lista::Seta, false),
        "tarefa" => (Lista::Tarefa, false),
        "numerica" => (Lista::Numerica, true),
        "alfabetica" => (Lista::Alfabetica, true),
        "romana" => (Lista::Romana, true),
        "romana_minuscula" => (Lista::RomanaMinuscula, true),
        _ => return Err(format!("Estilo de lista desconhecido: \"{nome}\".")),
    })
}

/// O estilo de um item, com a numeração padrão contando como `Numerica`.
fn lista_efetiva(ordenado: bool, lista: Lista) -> Lista {
    if ordenado && lista == Lista::Padrao {
        Lista::Numerica
    } else {
        lista
    }
}

/// Este bloco já é um item com este estilo de lista?
fn ja_e(b: &crate::modelo::Bloco, lista: Lista, ordenado: bool) -> bool {
    matches!(&b.conteudo, Conteudo::Item { ordenado: o, .. } if *o == ordenado && lista_efetiva(*o, b.atrib.lista) == lista_efetiva(ordenado, lista))
}

impl Sessao {
    /// Aplica `f` nos atributos de cada parágrafo de texto tocado pela seleção.
    pub(crate) fn mudar_atributos(&mut self, f: impl Fn(&mut Atrib)) -> Mudanca {
        let (mut t, _, _) = self.trabalho_da_selecao();
        let mut mudou = false;
        for b in t.blocos.iter_mut().filter(|b| tem_texto_corrido(&b.conteudo)) {
            let antes = b.atrib;
            f(&mut b.atrib);
            mudou |= antes != b.atrib;
        }
        if !mudou {
            return self.sem_mudanca();
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// Entrelinha como múltiplo (1,0 · 1,15 · 1,5 · 2,0 …); 0 volta à padrão.
    pub fn entrelinha_definir(&mut self, fator: f32) -> Result<Mudanca, String> {
        if !(0.0..=10.0).contains(&fator) {
            return Err(format!("Entrelinha inválida: {fator} (use de 0,5 a 10)."));
        }
        Ok(self.mudar_atributos(|a| a.entrelinha = fator))
    }

    /// Espaço antes e depois do parágrafo, em pontos; `None` volta ao do tipo de bloco.
    pub fn espaco_definir(&mut self, antes: Option<f32>, depois: Option<f32>) -> Result<Mudanca, String> {
        if [antes, depois].iter().flatten().any(|v| !(0.0..=200.0).contains(v)) {
            return Err("O espaço antes e depois vai de 0 a 200 pt.".to_string());
        }
        Ok(self.mudar_atributos(|a| {
            a.antes = antes;
            a.depois = depois;
        }))
    }

    /// Cor de fundo do parágrafo (`#RRGGBB`; vazio = sem fundo).
    pub fn fundo_definir(&mut self, cor: &str) -> Result<Mudanca, String> {
        let c = ler_cor(cor)?;
        Ok(self.mudar_atributos(|a| a.fundo = c))
    }

    /// Bordas do parágrafo: bits 1 esquerda, 2 direita, 4 em cima, 8 embaixo (0 tira todas).
    pub fn borda_definir(&mut self, lados: u8) -> Mudanca {
        self.mudar_atributos(|a| a.borda = lados & 15)
    }

    /// Faz dos parágrafos tocados itens de lista no estilo pedido (`padrao`, `circulo`, `quadrado`, `seta`,
    /// `tarefa`, `numerica`, `alfabetica`, `romana`); se já estavam assim, voltam a parágrafo.
    pub fn lista_estilo(&mut self, nome: &str) -> Result<Mudanca, String> {
        let (lista, ordenado) = lista_de(nome)?;
        let (mut t, _, _) = self.trabalho_da_selecao();
        let alvos: Vec<usize> = (0..t.blocos.len()).filter(|&k| tem_texto_corrido(&t.blocos[k].conteudo) && !matches!(t.blocos[k].conteudo, Conteudo::Codigo { .. } | Conteudo::Equacao { .. })).collect();
        if alvos.is_empty() {
            return Ok(self.sem_mudanca());
        }
        let desfazer = alvos.iter().all(|&k| ja_e(&t.blocos[k], lista, ordenado));
        for k in alvos {
            let b = &mut t.blocos[k];
            let tipo = if desfazer { "paragrafo" } else if ordenado { "item_ordenado" } else { "item" };
            if let Ok(novo) = converter(&b.conteudo, tipo) {
                b.conteudo = novo;
            }
            b.atrib.limpar_lista();
            if !desfazer {
                b.atrib.lista = lista;
            }
        }
        let cursor = self.cursor;
        Ok(self.fechar_trabalho(t, cursor, None))
    }

    /// Liga ou desliga "recomeçar a numeração aqui" nos itens numerados tocados.
    pub fn lista_reiniciar(&mut self) -> Mudanca {
        let (t, _, _) = self.trabalho_da_selecao();
        let liga = !t.blocos.iter().filter(|b| matches!(b.conteudo, Conteudo::Item { ordenado: true, .. })).all(|b| b.atrib.reiniciar);
        let (mut t, _, _) = self.trabalho_da_selecao();
        let mut mudou = false;
        for b in t.blocos.iter_mut().filter(|b| matches!(b.conteudo, Conteudo::Item { ordenado: true, .. })) {
            mudou |= b.atrib.reiniciar != liga;
            b.atrib.reiniciar = liga;
        }
        if !mudou {
            return self.sem_mudanca();
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// Marca ou desmarca a caixa do item de tarefa `id`.
    pub fn tarefa_alternar(&mut self, id: u64) -> Mudanca {
        let Some(i) = self.idx(id) else { return self.sem_mudanca() };
        let mut b = self.doc.blocos[i].clone();
        if !matches!(b.conteudo, Conteudo::Item { .. }) || b.atrib.lista != Lista::Tarefa {
            return self.sem_mudanca();
        }
        b.atrib.marcado = !b.atrib.marcado;
        let cursor = self.cursor;
        self.transacao(crate::sessao::Splice { pos: i, quantos: 1, dentro: vec![b] }, cursor, None)
    }
}

impl Sessao {
    /// Um clique na caixa de um item de tarefa a marca ou desmarca; `None` se o clique não foi numa caixa.
    pub fn clicar_tarefa(&mut self, pagina: usize, x: f32, y: f32) -> Option<Mudanca> {
        let p = self.pos_na_pagina(pagina, x, y)?;
        let i = self.idx(p.bloco)?;
        let b = &self.doc.blocos[i];
        let Conteudo::Item { nivel, .. } = b.conteudo else { return None };
        if b.atrib.lista != Lista::Tarefa || p.desl != 0 {
            return None;
        }
        let recuo = 18.0 + 18.0 * f32::from(nivel) + b.atrib.recuo_esq.max(0.0);
        let rel = x - self.doc.pagina.margem_esq;
        (recuo - 22.0..=recuo - 2.0).contains(&rel).then(|| self.tarefa_alternar(p.bloco))
    }
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

    #[test]
    fn entrelinha_e_espaco_mudam_a_altura() {
        let mut s = doc(&["abc"]);
        let h = s.layouts[0].linhas[0].altura;
        s.entrelinha_definir(2.0).unwrap();
        assert!(s.layouts[0].linhas[0].altura > h * 1.6);
        s.espaco_definir(Some(12.0), Some(0.0)).unwrap();
        assert_eq!((s.layouts[0].antes, s.layouts[0].depois), (12.0, 0.0));
        assert!(s.entrelinha_definir(99.0).is_err());
    }

    fn marcadores(s: &Sessao) -> Vec<String> {
        crate::paginacao::marcadores(&s.documento().blocos, &s.documento().estilos.listas).into_iter().map(Option::unwrap_or_default).collect()
    }

    #[test]
    fn numeracao_romana_alfabetica_e_reiniciar() {
        let mut s = doc(&["a", "b", "c"]);
        s.selecionar_tudo();
        s.lista_estilo("romana").unwrap();
        assert_eq!(marcadores(&s), ["I.", "II.", "III."]);
        s.lista_estilo("alfabetica").unwrap();
        assert_eq!(marcadores(&s), ["a)", "b)", "c)"]);
        s.mover(Movimento::InicioDoDocumento, false);
        s.mover(Movimento::Baixo, false);
        s.lista_reiniciar();
        assert_eq!(marcadores(&s), ["a)", "a)", "b)"]);
        s.selecionar_tudo();
        s.lista_estilo("alfabetica").unwrap();
        assert!(matches!(s.documento().blocos[0].conteudo, Conteudo::Paragrafo { .. }), "repetir o mesmo estilo desfaz a lista");
    }

    #[test]
    fn marcadores_de_bolinha_quadrado_e_seta() {
        let mut s = doc(&["a"]);
        for (nome, m) in [("circulo", "○"), ("quadrado", "▪"), ("seta", "►")] {
            s.lista_estilo(nome).unwrap();
            assert_eq!(marcadores(&s), [m]);
            s.lista_estilo(nome).unwrap();
        }
        assert!(s.lista_estilo("estrela").is_err());
    }

    #[test]
    fn tarefa_marca_e_o_enter_cria_item_desmarcado() {
        let mut s = doc(&["comprar pão"]);
        s.lista_estilo("tarefa").unwrap();
        let id = s.documento().blocos[0].id;
        s.tarefa_alternar(id);
        assert!(s.documento().blocos[0].atrib.marcado);
        assert!(s.layouts[0].linhas[0].deco.len() > 4, "caixa e visto");
        s.mover(Movimento::FimDoDocumento, false);
        s.quebrar_paragrafo();
        let novo = &s.documento().blocos[1];
        assert_eq!(novo.atrib.lista, Lista::Tarefa);
        assert!(!novo.atrib.marcado);
    }

    #[test]
    fn clicar_na_caixa_da_tarefa_marca_e_fora_dela_nao() {
        let mut s = doc(&["comprar"]);
        s.lista_estilo("tarefa").unwrap();
        s.mover(Movimento::InicioDoDocumento, false);
        let g = s.geometria(&s.cursor().foco).unwrap();
        assert!(s.clicar_tarefa(g.pagina, g.x + 40.0, g.y + 3.0).is_none(), "clicar no texto não marca");
        assert!(!s.documento().blocos[0].atrib.marcado);
        assert!(s.clicar_tarefa(g.pagina, g.x - 12.0, g.y + 3.0).is_some());
        assert!(s.documento().blocos[0].atrib.marcado);
    }

    #[test]
    fn fundo_e_bordas_desenham_na_linha() {
        let mut s = doc(&["abc"]);
        s.fundo_definir("#ffeeaa").unwrap();
        s.borda_definir(15);
        let deco = &s.layouts[0].linhas[0].deco;
        assert_eq!(deco.len(), 5);
        assert!(s.fundo_definir("rosa").is_err());
    }
}
