//! Faixa estreita no jeito do FeatherSheet (`faixa_responsiva.rs` de lá): os grupos encolhem por
//! prioridade, não pela posição. Primeiro todos os que têm versão compacta ficam compactos
//! (botões pequenos empilhados); só depois algum vira um botão só, com o ícone e a setinha, que
//! abre o grupo inteiro num menu. Assim nada transborda para uma barra de rolagem.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nivel {
    Completo,
    Compacto,
    Recolhido,
}

/// O que o planejamento precisa saber de um grupo: as larguras (medidas na tela) de cada nível e a
/// prioridade (menor encolhe antes; empate: o mais à direita antes).
#[derive(Clone, Copy, Debug)]
pub struct Medida {
    pub completo: f32,
    /// `None`: o grupo não tem versão compacta e passa direto a recolhido.
    pub compacto: Option<f32>,
    pub recolhido: f32,
    pub prioridade: u8,
}

impl Medida {
    fn largura(&self, n: Nivel) -> f32 {
        match n {
            Nivel::Completo => self.completo,
            Nivel::Compacto => self.compacto.unwrap_or(self.completo),
            Nivel::Recolhido => self.recolhido,
        }
    }
}

pub fn largura_total(grupos: &[Medida], niveis: &[Nivel]) -> f32 {
    grupos.iter().zip(niveis).map(|(g, &n)| g.largura(n)).sum()
}

/// O nível de cada grupo para caber em `disponivel` pixels. Se nem tudo recolhido couber, devolve
/// tudo recolhido (a faixa ainda rola, mas só numa janela menor que qualquer uso real).
pub fn planejar(grupos: &[Medida], disponivel: f32) -> Vec<Nivel> {
    let mut niveis = vec![Nivel::Completo; grupos.len()];
    let mut ordem: Vec<usize> = (0..grupos.len()).collect();
    ordem.sort_by_key(|&i| (grupos[i].prioridade, std::cmp::Reverse(i)));
    for alvo in [Nivel::Compacto, Nivel::Recolhido] {
        for &i in &ordem {
            if largura_total(grupos, &niveis) <= disponivel {
                return niveis;
            }
            if alvo == Nivel::Recolhido || grupos[i].compacto.is_some() {
                niveis[i] = alvo;
            }
        }
    }
    niveis
}

#[cfg(test)]
mod testes {
    use super::*;

    fn medidas() -> Vec<Medida> {
        vec![
            Medida { completo: 180., compacto: Some(110.), recolhido: 70., prioridade: 2 },
            Medida { completo: 400., compacto: Some(260.), recolhido: 70., prioridade: 9 },
            Medida { completo: 300., compacto: Some(200.), recolhido: 70., prioridade: 5 },
            Medida { completo: 450., compacto: Some(240.), recolhido: 70., prioridade: 1 },
            Medida { completo: 120., compacto: None, recolhido: 70., prioridade: 1 },
        ]
    }

    #[test]
    fn largo_fica_inteiro_e_estreito_cabe() {
        let g = medidas();
        assert!(planejar(&g, 2000.).iter().all(|&n| n == Nivel::Completo));
        for w in [1400., 1100., 900., 700., 500., 360.] {
            let n = planejar(&g, w);
            assert!(largura_total(&g, &n) <= w, "{w}px não coube: {n:?}");
        }
    }

    #[test]
    fn a_prioridade_decide_quem_encolhe_primeiro() {
        let g = medidas();
        // Falta pouco: só o de menor prioridade com versão compacta (o 3) encolhe.
        let n = planejar(&g, 1400.);
        assert_eq!(n[3], Nivel::Compacto);
        assert_eq!(n[1], Nivel::Completo, "o de maior prioridade é o último");
        // Todos compactos antes de qualquer um recolher.
        let n = planejar(&g, 900.);
        assert!(n.iter().all(|&x| x != Nivel::Recolhido) || n.iter().all(|&x| x != Nivel::Completo));
    }

    #[test]
    fn encolher_a_janela_nunca_aumenta_um_grupo() {
        let g = medidas();
        let indice = |n: Nivel| n as u8;
        let mut antes = planejar(&g, 2000.);
        for w in (300..2000).rev().step_by(10) {
            let agora = planejar(&g, w as f32);
            for (a, b) in antes.iter().zip(&agora) {
                assert!(indice(*b) >= indice(*a), "{w}px: um grupo voltou a crescer");
            }
            antes = agora;
        }
        assert!(planejar(&g, 10.).iter().all(|&n| n == Nivel::Recolhido));
    }
}
