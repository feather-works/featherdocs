//! Adaptador vetorial entre o quadro binário FeatherDocs e Drawing.
//!
//! Unidades continuam em pontos de documento. Não transfere texto/imagens
//! para uma cena incompleta: exportação SVG cobre somente os vetores.
//! A interface consome as MESMAS figuras, mantendo z-order do quadro original.
//! Isto não altera paginação, cursor, seleção nem os arquivos .fdoz/.docx.

use crate::quadro::{Operacao, Quadro};
use feather_draw::{Cena, Cor, Estilo, Figura, Ponto, Retangulo};
use crate::compat::canvas::{Frame, Path, Stroke};
use pelicano::{Color, Point, Size};

fn cor_drawing(valor: u32) -> Cor {
    Cor::rgba(
        (valor >> 24) as u8,
        (valor >> 16) as u8,
        (valor >> 8) as u8,
        valor as u8,
    )
}

fn cor_tela(c: Cor) -> Color {
    Color::rgb8(c.r, c.g, c.b).alpha(f32::from(c.a) / 255.0)
}

/// Converte somente vetores do contrato de quadro; textos/imagens
/// permanecem sob renderização e exportação específicas do FeatherDocs.
pub fn figura_vetorial(op: &Operacao) -> Option<Figura> {
    match *op {
        Operacao::Retangulo { rect: [x, y, w, h], cor }
            if [x, y, w, h].iter().all(|v| v.is_finite()) && w > 0.0 && h > 0.0 =>
        {
            Some(Figura::Retangulo {
                area: Retangulo::novo(x, y, w, h),
                estilo: Estilo {
                    preenchimento: Some(cor_drawing(cor)),
                    contorno: None,
                    espessura: 1.0,
                },
            })
        }
        Operacao::Traco { pontos: [x, y, a, b, largura], cor }
            if [x, y, a, b, largura].iter().all(|v| v.is_finite()) && largura > 0.0 =>
        {
            Some(Figura::Polilinha {
                pontos: vec![Ponto::novo(x, y), Ponto::novo(a, b)],
                fechada: false,
                estilo: Estilo {
                    preenchimento: None,
                    contorno: Some(cor_drawing(cor)),
                    espessura: largura,
                },
            })
        }
        _ => None,
    }
}

/// Extrai, na ordem de desenho original, os vetores de uma página.
pub fn cena_da_pagina(quadro: &Quadro) -> Cena {
    let mut cena = Cena::nova(quadro.largura, quadro.altura);
    for op in &quadro.operacoes {
        if let Some(figura) = figura_vetorial(op) {
            cena.adicionar(figura);
        }
    }
    cena
}

/// Renderiza no canvas usando a geometria e os estilos da figura Drawing.
/// Retorna false para operações que continuam exclusivas do motor Docs.
pub fn desenhar_vetor(frame: &mut Frame, op: &Operacao) -> bool {
    match figura_vetorial(op) {
        Some(Figura::Retangulo { area, estilo }) => {
            if let Some(c) = estilo.preenchimento {
                frame.fill_rectangle(
                    Point::new(area.x, area.y),
                    Size::new(area.largura, area.altura),
                    cor_tela(c),
                );
            }
            true
        }
        Some(Figura::Polilinha { pontos, estilo, .. }) => {
            if let (Some(c), [a, b]) = (estilo.contorno, pontos.as_slice()) {
                frame.stroke(
                    &Path::line(Point::new(a.x, a.y), Point::new(b.x, b.y)),
                    Stroke::default().with_color(cor_tela(c)).with_width(estilo.espessura),
                );
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn quadro() -> Quadro {
        Quadro {
            largura: 612.0,
            altura: 792.0,
            operacoes: vec![
                Operacao::Retangulo { rect: [10.0, 20.0, 40.0, 30.0], cor: 0xff000080 },
                Operacao::Texto {
                    x: 12.0, y: 16.0, tamanho: 11.0, estilo: 0,
                    cor: 0xff0000ff, texto: "não exportar texto como vetor".into(), angulo: None,
                },
                Operacao::Traco { pontos: [1.0, 2.0, 21.0, 2.0, 0.5], cor: 0x0000ffff },
            ],
        }
    }

    #[test]
    fn preserva_cor_alfa_espessura_z_order_e_unidade_pontos() {
        let cena = cena_da_pagina(&quadro());
        assert_eq!((cena.largura, cena.altura, cena.figuras.len()), (612.0, 792.0, 2));
        match &cena.figuras[0] {
            Figura::Retangulo { area, estilo } => {
                assert_eq!((area.x, area.y, area.largura, area.altura), (10.0, 20.0, 40.0, 30.0));
                assert_eq!(estilo.preenchimento, Some(Cor::rgba(255, 0, 0, 128)));
            }
            _ => panic!("retângulo ausente"),
        }
        match &cena.figuras[1] {
            Figura::Polilinha { pontos, fechada, estilo } => {
                assert_eq!(pontos.as_slice(), &[Ponto::novo(1.0, 2.0), Ponto::novo(21.0, 2.0)]);
                assert!(!*fechada);
                assert_eq!(estilo.contorno, Some(Cor::rgba(0, 0, 255, 255)));
                assert_eq!(estilo.espessura, 0.5);
            }
            _ => panic!("traço ausente"),
        }
        let svg = feather_draw::exportar_svg(&cena).expect("Cena válida");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("#ff0000"));
        assert!(svg.contains("#0000ff"));
    }

    #[test]
    fn vetor_invalido_nao_contamina_exportacao() {
        let mut q = quadro();
        q.operacoes.push(Operacao::Traco {
            pontos: [f32::NAN, 0.0, 1.0, 1.0, 2.0],
            cor: 0,
        });
        q.operacoes.push(Operacao::Retangulo {
            rect: [0.0, 0.0, -10.0, 1.0],
            cor: 0,
        });
        assert_eq!(cena_da_pagina(&q).figuras.len(), 2);
    }
}
