//! Formatar o que está selecionado (negrito, itálico…), mudar o estilo do parágrafo,
//! listas, recuo, inserir tabela/sumário/linha/quebra, e o atalho Markdown que se
//! digita (`# `, `- `, `1. `, `> `, `---`, ```` ``` ````, `[sumario]`).

use serde::Serialize;

use crate::alvo::{com_texto_ref, dimensoes, eh_atomico, tamanho_alvo, texto_alvo};
use crate::cursor::{Cursor, Pos};
use crate::digitacao::{tem_texto_corrido, Trabalho};
use crate::edicao::{converter, nome_do_tipo};
use crate::modelo::{Alinhamento, Bloco, Conteudo, CODIGO, PASSO_RECUO, ITALICO, NEGRITO, SUBLINHADO, TACHADO};
use crate::sessao::{Mudanca, Sessao, Splice};
use crate::fonte::cor_em_texto;
use crate::modelo::{Fmt, Lista, SERIFA};
use crate::texto::{aplicar_estilo, fatia, marca_em, plano, todo_com_estilo};

/// O que a barra de ferramentas e os menus mostram sobre o ponto onde o cursor está.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Formato {
    pub negrito: bool,
    pub italico: bool,
    pub sublinhado: bool,
    pub tachado: bool,
    pub codigo: bool,
    /// Alinhamento do parágrafo no cursor (`esquerda`, `centro`, `direita`, `justificado`).
    pub alinhamento: String,
    /// Recuo esquerdo do parágrafo no cursor, em pontos.
    pub recuo: f32,
    /// Recuo direito e da primeira linha, em pontos.
    pub recuo_dir: f32,
    pub primeira: f32,
    /// `sans`, `serifa` ou `mono`.
    pub fonte: String,
    /// O corpo em pontos no cursor (o do trecho ou o do tipo de bloco).
    pub tamanho: f32,
    /// `#rrggbb` ou vazio (automática).
    pub cor: String,
    pub realce: String,
    pub sobrescrito: bool,
    pub subscrito: bool,
    /// Entrelinha efetiva (múltiplo) e espaço antes/depois do parágrafo, em pontos.
    pub entrelinha: f32,
    pub antes: f32,
    pub depois: f32,
    pub fundo: String,
    pub borda: u8,
    /// O estilo de lista do bloco (`padrao`, `circulo`…) e se a tarefa está marcada.
    pub lista: String,
    pub marcado: bool,
    pub pincel: bool,
    /// O endereço do link no cursor (para a caixa de link vir preenchida).
    pub link: Option<String>,
    /// A tabela do cursor tem linha de cabeçalho.
    pub tabela_cabecalho: bool,
    /// A chamada do parágrafo (`info`, `dica`, `aviso`, `atencao`) ou vazio.
    pub chamada: String,
    /// O tipo do bloco no cursor (`paragrafo`, `titulo2`, `item`…).
    pub estilo: String,
    /// O estilo com nome do parágrafo no cursor (0 = só o de fábrica).
    pub estilo_proprio: u32,
    /// O `id` do bloco do cursor (o modo dividido marca as linhas dele no código).
    pub bloco: u64,
    /// O título no cursor está fora da numeração automática.
    pub sem_numero: bool,
    pub tem_selecao: bool,
    /// O `id` da tabela, se o cursor está numa célula (para "Editar no FeatherSheet").
    pub tabela: Option<u64>,
}

/// Uma parte de texto a formatar: o bloco (índice no trabalho), a célula e o trecho `[a, b)`.
pub(crate) struct Faixa {
    pub bloco: usize,
    pub celula: Option<(u32, u32)>,
    pub a: usize,
    pub b: usize,
}

/// Os trechos de texto que a seleção cobre, bloco a bloco.
pub(crate) fn faixas(t: &Trabalho, ini: Pos, fim: Pos) -> Vec<Faixa> {
    let ultimo = t.blocos.len() - 1;
    let mut saida = Vec::new();
    for (k, b) in t.blocos.iter().enumerate() {
        let c = &b.conteudo;
        if eh_atomico(c) {
            continue;
        }
        if let (Conteudo::Tabela { .. }, Some((linhas, colunas))) = (c, dimensoes(c)) {
            if t.blocos.len() == 1 {
                saida.push(Faixa { bloco: k, celula: ini.celula, a: ini.desl, b: fim.desl });
            } else {
                for l in 0..linhas {
                    for col in 0..colunas {
                        let cel = Some((l as u32, col as u32));
                        saida.push(Faixa { bloco: k, celula: cel, a: 0, b: tamanho_alvo(c, cel) });
                    }
                }
            }
            continue;
        }
        let a = if k == 0 { ini.desl } else { 0 };
        let b = if k == ultimo { fim.desl } else { tamanho_alvo(c, None) };
        saida.push(Faixa { bloco: k, celula: None, a, b });
    }
    saida
}

/// O tipo da chamada pela cor da barra (as cores estão em `insercao.rs`).
fn nome_da_chamada(cor: u32) -> &'static str {
    match cor {
        0x1a73_e8ff => "info",
        0x1e8e_3eff => "dica",
        0xf9ab_00ff => "aviso",
        0xd930_25ff => "atencao",
        _ => "",
    }
}

fn nome_da_lista(l: Lista) -> &'static str {
    match l {
        Lista::Padrao => "padrao",
        Lista::Circulo => "circulo",
        Lista::Quadrado => "quadrado",
        Lista::Seta => "seta",
        Lista::Tarefa => "tarefa",
        Lista::Numerica => "numerica",
        Lista::Alfabetica => "alfabetica",
        Lista::Romana => "romana",
        Lista::RomanaMinuscula => "romana_minuscula",
    }
}

impl Sessao {
    /// Liga ou desliga `bit` (negrito, itálico…) na seleção; sem seleção, vale para o próximo texto digitado.
    pub fn formatar(&mut self, bit: u8) -> Mudanca {
        if self.cursor.vazio() {
            let (atual, fmt) = self.marca_no_cursor();
            self.cursor.pendente = Some((atual ^ bit, fmt));
            return self.sem_mudanca();
        }
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        let partes = faixas(&t, ini, fim);
        let todos = partes.iter().filter(|f| f.b > f.a).all(|f| texto_alvo(&t.blocos[f.bloco].conteudo, f.celula).is_some_and(|x| todo_com_estilo(&x, f.a, f.b, bit)));
        for f in &partes {
            com_texto_ref(&mut t.blocos[f.bloco].conteudo, f.celula, |x| aplicar_estilo(x, f.a, f.b, bit, !todos));
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// A formatação que o texto digitado agora receberia.
    pub(crate) fn marca_no_cursor(&self) -> (u8, Fmt) {
        if let Some(p) = self.cursor.pendente {
            return p;
        }
        let f = self.cursor.foco;
        self.idx(f.bloco).and_then(|i| texto_alvo(&self.doc.blocos[i].conteudo, f.celula)).map_or((0, Fmt::default()), |t| marca_em(&t, f.desl))
    }

    fn estilo_no_cursor(&self) -> u8 {
        self.marca_no_cursor().0
    }

    /// O estado da formatação no cursor (para pintar os botões da barra).
    pub fn formato_do_cursor(&self) -> Formato {
        let f = self.cursor.foco;
        let i = self.idx(f.bloco).unwrap_or(0);
        let marca = self.marca_da_selecao();
        // A fonte e a cor de base vêm do estilo com nome do parágrafo (os botões B/I/U seguem só o trecho,
        // como nos títulos: eles ligam e desligam a formatação feita à mão).
        let do_estilo = self.doc.estilos.resolvido_do_bloco(&self.doc.blocos[i]).map_or((0, 0), |e| (e.bits, e.cor));
        let tem = |bit: u8| -> bool {
            if self.cursor.vazio() {
                return self.estilo_no_cursor() & bit != 0;
            }
            let (t, ini, fim) = self.trabalho_da_selecao();
            faixas(&t, ini, fim).iter().filter(|x| x.b > x.a).all(|x| texto_alvo(&t.blocos[x.bloco].conteudo, x.celula).is_some_and(|y| todo_com_estilo(&y, x.a, x.b, bit)))
        };
        Formato {
            negrito: tem(NEGRITO),
            italico: tem(ITALICO),
            sublinhado: tem(SUBLINHADO),
            tachado: tem(TACHADO),
            codigo: tem(CODIGO),
            alinhamento: match self.doc.blocos[i].atrib.alinhamento {
                Alinhamento::Esquerda => "esquerda",
                Alinhamento::Centro => "centro",
                Alinhamento::Direita => "direita",
                Alinhamento::Justificado => "justificado",
            }
            .to_string(),
            recuo: self.doc.blocos[i].atrib.recuo_esq,
            recuo_dir: self.doc.blocos[i].atrib.recuo_dir,
            primeira: self.doc.blocos[i].atrib.primeira,
            fonte: if (marca.0 | do_estilo.0) & CODIGO != 0 { "mono" } else if (marca.0 | do_estilo.0) & SERIFA != 0 { "serifa" } else { "sans" }.to_string(),
            tamanho: self.tamanho_no_cursor(),
            cor: cor_em_texto(if marca.1.cor != 0 { marca.1.cor } else { do_estilo.1 }),
            realce: cor_em_texto(marca.1.realce),
            sobrescrito: marca.1.posicao > 0,
            subscrito: marca.1.posicao < 0,
            entrelinha: if self.doc.blocos[i].atrib.entrelinha > 0.0 { self.doc.blocos[i].atrib.entrelinha } else { 1.15 },
            antes: self.layouts.get(i).map_or(0.0, |l| l.antes),
            depois: self.layouts.get(i).map_or(0.0, |l| l.depois),
            fundo: cor_em_texto(self.doc.blocos[i].atrib.fundo),
            borda: self.doc.blocos[i].atrib.borda,
            lista: nome_da_lista(self.doc.blocos[i].atrib.lista).to_string(),
            marcado: self.doc.blocos[i].atrib.marcado,
            pincel: self.pincel.is_some(),
            link: self.link_no_cursor(),
            tabela_cabecalho: self.tabela_tem_cabecalho(),
            chamada: nome_da_chamada(self.doc.blocos[i].atrib.cor_borda).to_string(),
            estilo: nome_do_tipo(&self.doc.blocos[i].conteudo),
            estilo_proprio: self.estilo_com_nome_no_cursor(),
            bloco: f.bloco,
            sem_numero: self.doc.blocos[i].atrib.sem_numero,
            tem_selecao: !self.cursor.vazio(),
            tabela: f.celula.map(|_| f.bloco),
        }
    }

    /// Muda o estilo dos parágrafos tocados pela seleção (`paragrafo`, `titulo1`…, `item`, `item_ordenado`, `citacao`,
    /// `codigo`, `equacao`). Repetir `item`, `item_ordenado` ou `citacao` desfaz (volta a parágrafo). `tabela`, `sumario`,
    /// `regua` e `quebra_de_pagina` não são estilo: inserem um bloco.
    pub fn paragrafo_estilo(&mut self, tipo: &str) -> Mudanca {
        if matches!(tipo, "tabela" | "sumario" | "regua" | "quebra_de_pagina" | "quebra_de_coluna" | "secao_proxima" | "secao_continua" | "secao_par" | "secao_impar") {
            return self.inserir_bloco(tipo);
        }
        let (mut t, _, _) = self.trabalho_da_selecao();
        let alvo = if t.blocos.iter().filter(|b| tem_texto_corrido(&b.conteudo)).all(|b| nome_do_tipo(&b.conteudo) == tipo) && matches!(tipo, "item" | "item_ordenado" | "citacao") { "paragrafo" } else { tipo };
        let mut mudou = false;
        for b in &mut t.blocos {
            if !tem_texto_corrido(&b.conteudo) {
                continue;
            }
            if let Ok(novo) = converter(&b.conteudo, alvo) {
                if novo != b.conteudo {
                    b.conteudo = novo;
                    b.atrib.limpar_lista();
                    mudou = true;
                }
                // O estilo de fábrica tira o estilo com nome.
                if b.atrib.estilo != 0 {
                    b.atrib.estilo = 0;
                    mudou = true;
                }
            }
        }
        if !mudou {
            return self.sem_mudanca();
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// Recua (`+1`) ou avança (`-1`) os itens de lista tocados pela seleção.
    pub fn mudar_nivel(&mut self, delta: i32) -> Mudanca {
        let (mut t, _, _) = self.trabalho_da_selecao();
        let mut mudou = false;
        for b in &mut t.blocos {
            if let Conteudo::Item { nivel, .. } = &mut b.conteudo {
                let novo = (i32::from(*nivel) + delta).clamp(0, 5) as u8;
                mudou |= novo != *nivel;
                *nivel = novo;
            }
        }
        if !mudou {
            return self.sem_mudanca();
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// Alinha os parágrafos tocados pela seleção (`esquerda`, `centro`, `direita`, `justificado`).
    pub fn alinhar(&mut self, como: &str) -> Result<Mudanca, String> {
        let novo = match como {
            "esquerda" => Alinhamento::Esquerda,
            "centro" => Alinhamento::Centro,
            "direita" => Alinhamento::Direita,
            "justificado" => Alinhamento::Justificado,
            _ => return Err(format!("Alinhamento desconhecido: \"{como}\".")),
        };
        Ok(self.mudar_atrib(|a| a.alinhamento = novo))
    }

    /// Aumenta (`+1`) ou diminui (`-1`) o recuo, em passos de 1,25 cm: nos itens de lista muda o nível;
    /// nos demais parágrafos, o recuo esquerdo.
    pub fn recuar(&mut self, delta: i32) -> Mudanca {
        let (mut t, _, _) = self.trabalho_da_selecao();
        let mut mudou = false;
        for b in &mut t.blocos {
            match &mut b.conteudo {
                Conteudo::Item { nivel, .. } => {
                    let novo = (i32::from(*nivel) + delta).clamp(0, 5) as u8;
                    mudou |= novo != *nivel;
                    *nivel = novo;
                }
                c if tem_texto_corrido(c) => {
                    let novo = (b.atrib.recuo_esq + delta as f32 * PASSO_RECUO).clamp(0.0, 10.0 * PASSO_RECUO);
                    mudou |= (novo - b.atrib.recuo_esq).abs() > f32::EPSILON;
                    b.atrib.recuo_esq = novo;
                }
                _ => {}
            }
        }
        if !mudou {
            return self.sem_mudanca();
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// Define o recuo esquerdo, o direito e o da primeira linha (pontos) dos parágrafos tocados (a régua).
    pub fn recuos_definir(&mut self, esq: f32, dir: f32, primeira: f32) -> Mudanca {
        self.mudar_atrib(|a| {
            a.recuo_esq = esq.max(0.0);
            a.recuo_dir = dir.max(0.0);
            a.primeira = primeira;
        })
    }

    /// Aplica `f` nos atributos de cada parágrafo de texto tocado pela seleção.
    fn mudar_atrib(&mut self, f: impl Fn(&mut crate::modelo::Atrib)) -> Mudanca {
        let (mut t, _, _) = self.trabalho_da_selecao();
        let mut mudou = false;
        for b in &mut t.blocos {
            if !tem_texto_corrido(&b.conteudo) {
                continue;
            }
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

    /// Tab na tabela: a célula seguinte (ou a anterior); no fim da última, acrescenta uma linha.
    pub(crate) fn celula_seguinte(&mut self, i: usize, voltar: bool) -> Mudanca {
        let foco = self.cursor.foco;
        let (Some((l, k)), Some((linhas, colunas))) = (foco.celula, dimensoes(&self.doc.blocos[i].conteudo)) else { return self.sem_mudanca() };
        let (l, k) = (l as usize, k as usize);
        let destino = if voltar {
            if k > 0 { Some((l, k - 1)) } else if l > 0 { Some((l - 1, colunas - 1)) } else { None }
        } else if k + 1 < colunas {
            Some((l, k + 1))
        } else {
            Some((l + 1, 0))
        };
        let Some((dl, dk)) = destino else { return self.sem_mudanca() };
        if dl >= linhas {
            return self.acrescentar_linha(i, colunas);
        }
        let cel = Some((dl as u32, dk as u32));
        let total = tamanho_alvo(&self.doc.blocos[i].conteudo, cel);
        self.cursor = Cursor { ancora: Pos { bloco: foco.bloco, celula: cel, desl: 0 }, foco: Pos { bloco: foco.bloco, celula: cel, desl: total }, x_desejado: None, pendente: None };
        self.digitando = None;
        self.sem_mudanca()
    }

    fn acrescentar_linha(&mut self, i: usize, colunas: usize) -> Mudanca {
        let mut bloco = self.doc.blocos[i].clone();
        let Conteudo::Tabela { linhas, .. } = &mut bloco.conteudo else { return self.sem_mudanca() };
        linhas.push(vec![Vec::new(); colunas]);
        let nova = linhas.len() as u32 - 1;
        let cursor = Cursor::em(Pos { bloco: bloco.id, celula: Some((nova, 0)), desl: 0 });
        self.transacao(Splice { pos: i, quantos: 1, dentro: vec![bloco] }, cursor, None)
    }

    /// Insere um bloco depois do parágrafo do cursor (ou no lugar dele, se está vazio):
    /// `tabela`, `sumario`, `regua`, `quebra_de_pagina`, `codigo`, `equacao`, `citacao`.
    pub fn inserir_bloco(&mut self, tipo: &str) -> Mudanca {
        let Ok(novo) = converter(&Conteudo::Paragrafo { texto: Vec::new() }, tipo) else { return self.sem_mudanca() };
        self.inserir_conteudo(novo)
    }

    /// Insere `novo` depois do parágrafo do cursor (ou no lugar dele, se está vazio), e deixa onde escrever depois.
    pub(crate) fn inserir_conteudo(&mut self, novo: Conteudo) -> Mudanca {
        let foco = self.cursor.foco;
        let Some(i) = self.idx(foco.bloco) else { return self.sem_mudanca() };
        let vazio = matches!(&self.doc.blocos[i].conteudo, Conteudo::Paragrafo { texto } if texto.is_empty());
        let (pos, quantos) = if vazio { (i, 1) } else { (i + 1, 0) };
        let bloco = if vazio { Bloco::novo(foco.bloco, novo) } else { self.doc.novo_bloco(novo) };
        let eh_texto = !eh_atomico(&bloco.conteudo);
        let cursor_no_bloco = Pos { bloco: bloco.id, celula: crate::alvo::primeiro_alvo(&bloco.conteudo).flatten(), desl: 0 };
        let mut dentro = vec![bloco];
        let fim_do_documento = pos + quantos >= self.doc.blocos.len();
        // Depois de um bloco sem texto (ou de uma tabela) tem que haver onde escrever.
        let precisa_de_paragrafo = !eh_texto || matches!(dentro[0].conteudo, Conteudo::Tabela { .. });
        let mut cursor = cursor_no_bloco;
        if precisa_de_paragrafo && fim_do_documento {
            let p = self.doc.novo_bloco(Conteudo::Paragrafo { texto: Vec::new() });
            if !eh_texto {
                cursor = Pos { bloco: p.id, celula: None, desl: 0 };
            }
            dentro.push(p);
        } else if !eh_texto {
            cursor = self.inicio_do_bloco(pos + quantos);
        }
        self.transacao(Splice { pos, quantos, dentro }, Cursor::em(cursor), None)
    }

    /// `# `, `- `, `1. `, `> ` digitados no começo de um parágrafo viram título, lista ou citação ao apertar Espaço.
    pub(crate) fn autoformatar_espaco(&mut self) -> Option<Mudanca> {
        let foco = self.cursor.foco;
        let i = self.idx(foco.bloco)?;
        let Conteudo::Paragrafo { texto } = &self.doc.blocos[i].conteudo else { return None };
        let antes = plano(&fatia(texto, 0, foco.desl));
        let resto = fatia(texto, foco.desl, usize::MAX);
        let novo = match antes.as_str() {
            "-" | "*" | "+" => Conteudo::Item { ordenado: false, nivel: 0, texto: resto },
            ">" => Conteudo::Citacao { texto: resto },
            m if (1..=6).contains(&m.len()) && m.chars().all(|c| c == '#') => Conteudo::Titulo { nivel: m.len() as u8, texto: resto },
            m if m.len() >= 2 && m.len() <= 10 && m.ends_with('.') && m[..m.len() - 1].chars().all(|c| c.is_ascii_digit()) => Conteudo::Item { ordenado: true, nivel: 0, texto: resto },
            _ => return None,
        };
        let cursor = Cursor::em(Pos { bloco: foco.bloco, celula: None, desl: 0 });
        Some(self.transacao(Splice { pos: i, quantos: 1, dentro: vec![Bloco { atrib: self.doc.blocos[i].atrib, ..Bloco::novo(foco.bloco, novo) }] }, cursor, None))
    }

    /// `---`, ```` ``` ```` e `[sumario]` seguidos de Enter viram linha, bloco de código e sumário.
    pub(crate) fn autoformatar_enter(&mut self) -> Option<Mudanca> {
        let foco = self.cursor.foco;
        let i = self.idx(foco.bloco)?;
        let Conteudo::Paragrafo { texto } = &self.doc.blocos[i].conteudo else { return None };
        let p = plano(texto);
        if foco.desl != p.chars().count() {
            return None;
        }
        let (novo, depois) = match p.trim() {
            "---" => (Conteudo::Regua, true),
            "[sumario]" | "[toc]" => (Conteudo::sumario(3), true),
            m if m.starts_with("```") && !m.contains(' ') => (Conteudo::Codigo { linguagem: m[3..].to_string(), codigo: String::new() }, false),
            _ => return None,
        };
        let mut dentro = vec![Bloco::novo(foco.bloco, novo)];
        let mut cursor = Pos { bloco: foco.bloco, celula: None, desl: 0 };
        if depois {
            let b = self.doc.novo_bloco(Conteudo::Paragrafo { texto: Vec::new() });
            cursor = Pos { bloco: b.id, celula: None, desl: 0 };
            dentro.push(b);
        }
        Some(self.transacao(Splice { pos: i, quantos: 1, dentro }, Cursor::em(cursor), None))
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

    fn tipos(s: &Sessao) -> Vec<String> {
        s.documento().blocos.iter().map(|b| nome_do_tipo(&b.conteudo)).collect()
    }

    #[test]
    fn negrito_na_selecao_e_de_novo_desliga() {
        let mut s = doc(&["abcdef"]);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Direita, true);
        s.mover(Movimento::Direita, true);
        s.formatar(NEGRITO);
        assert!(s.formato_do_cursor().negrito);
        let Conteudo::Paragrafo { texto } = &s.documento().blocos[0].conteudo else { panic!() };
        assert_eq!(texto.len(), 3);
        s.formatar(NEGRITO);
        assert!(!s.formato_do_cursor().negrito);
    }

    #[test]
    fn negrito_sem_selecao_vale_para_o_que_vem() {
        let mut s = doc(&[""]);
        s.formatar(NEGRITO);
        s.inserir_texto("oi");
        s.formatar(NEGRITO);
        s.inserir_texto("!");
        let Conteudo::Paragrafo { texto } = &s.documento().blocos[0].conteudo else { panic!() };
        assert_eq!((texto[0].estilo, texto[1].estilo), (NEGRITO, 0));
    }

    #[test]
    fn formatar_atravessa_paragrafos() {
        let mut s = doc(&["um", "dois"]);
        s.selecionar_tudo();
        s.formatar(ITALICO);
        for b in &s.documento().blocos {
            let Conteudo::Paragrafo { texto } = &b.conteudo else { panic!() };
            assert!(texto.iter().all(|t| t.estilo & ITALICO != 0));
        }
    }

    #[test]
    fn atalhos_ao_digitar_espaco_e_enter() {
        let mut s = doc(&[""]);
        s.inserir_texto("#");
        s.inserir_texto(" ");
        assert_eq!(tipos(&s), vec!["titulo1"]);
        s.inserir_texto("T");
        s.quebrar_paragrafo();
        s.inserir_texto("-");
        s.inserir_texto(" ");
        s.inserir_texto("a");
        s.quebrar_paragrafo();
        s.quebrar_paragrafo();
        assert_eq!(tipos(&s), vec!["titulo1", "item", "paragrafo"], "Enter no item vazio sai da lista");
        s.inserir_texto("---");
        s.quebrar_paragrafo();
        assert_eq!(tipos(&s), vec!["titulo1", "item", "regua", "paragrafo"]);
        s.inserir_texto("[sumario]");
        s.quebrar_paragrafo();
        assert!(tipos(&s).contains(&"sumario".to_string()));
    }

    #[test]
    fn enter_no_titulo_vira_paragrafo_e_no_item_continua_a_lista() {
        let mut s = doc(&["titulo"]);
        s.paragrafo_estilo("titulo2");
        s.mover(Movimento::FimDaLinha, false);
        s.quebrar_paragrafo();
        assert_eq!(tipos(&s), vec!["titulo2", "paragrafo"]);
        s.paragrafo_estilo("item_ordenado");
        s.inserir_texto("x");
        s.quebrar_paragrafo();
        assert_eq!(tipos(&s), vec!["titulo2", "item_ordenado", "item_ordenado"]);
    }

    #[test]
    fn estilo_de_paragrafo_alterna_lista() {
        let mut s = doc(&["a", "b"]);
        s.selecionar_tudo();
        s.paragrafo_estilo("item");
        assert_eq!(tipos(&s), vec!["item", "item"]);
        s.paragrafo_estilo("item");
        assert_eq!(tipos(&s), vec!["paragrafo", "paragrafo"]);
    }

    #[test]
    fn tab_recua_item_e_anda_na_tabela() {
        let mut s = doc(&["a"]);
        s.paragrafo_estilo("item");
        s.tab(false);
        assert!(matches!(s.documento().blocos[0].conteudo, Conteudo::Item { nivel: 1, .. }));
        s.tab(true);
        assert!(matches!(s.documento().blocos[0].conteudo, Conteudo::Item { nivel: 0, .. }));
        let mut s = doc(&[""]);
        s.inserir_bloco("tabela");
        assert_eq!(s.cursor.foco.celula, Some((0, 0)));
        for _ in 0..8 {
            s.tab(false);
        }
        assert_eq!(s.cursor.foco.celula, Some((2, 2)));
        s.tab(false);
        assert_eq!(s.cursor.foco.celula, Some((3, 0)), "Tab na última célula acrescenta uma linha");
        s.inserir_texto("x");
        s.desfazer().unwrap();
        s.desfazer().unwrap();
        assert!(matches!(&s.documento().blocos[0].conteudo, Conteudo::Tabela { linhas, .. } if linhas.len() == 3));
    }

    #[test]
    fn quebra_de_secao_em_pagina_impar_deixa_uma_em_branco_e_a_continua_nao_quebra() {
        let mut s = doc(&["a"]);
        s.inserir_bloco("secao_par");
        assert_eq!(s.total_de_paginas(), 2, "a página 2 já é par");
        let mut s = doc(&["a"]);
        s.inserir_bloco("secao_impar");
        assert_eq!(s.total_de_paginas(), 3, "a 2 fica em branco para a seção começar na 3");
        let mut s = doc(&["a"]);
        s.inserir_bloco("secao_continua");
        assert_eq!(s.total_de_paginas(), 1);
        s.inserir_bloco("quebra_de_coluna");
        assert_eq!(s.total_de_paginas(), 2, "com uma coluna, a quebra de coluna vai para a página seguinte");
    }

    #[test]
    fn quebra_de_linha_termina_a_linha_sem_dividir_o_paragrafo() {
        let mut s = doc(&["a"]);
        s.inserir_texto(&crate::modelo::QUEBRA_DE_LINHA.to_string());
        s.inserir_texto("b");
        assert_eq!(tipos(&s), vec!["paragrafo"]);
        assert_eq!(s.layouts[0].linhas.len(), 2);
    }

    #[test]
    fn inserir_bloco_no_lugar_do_paragrafo_vazio_e_deixa_onde_escrever() {
        let mut s = doc(&[""]);
        s.inserir_bloco("regua");
        assert_eq!(tipos(&s), vec!["regua", "paragrafo"]);
        s.inserir_texto("depois");
        assert_eq!(s.cursor.foco.desl, 6);
    }

    #[test]
    fn alinhar_centraliza_e_o_enter_herda() {
        let mut s = doc(&["abc"]);
        s.alinhar("centro").unwrap();
        assert_eq!(s.formato_do_cursor().alinhamento, "centro");
        let x0 = s.paginas[0].fatias.len();
        assert!(x0 > 0);
        let p = &s.layouts[0].linhas[0].pedacos[0];
        assert!(p.x > 100.0, "texto centrado começa longe da margem: {}", p.x);
        s.quebrar_paragrafo();
        assert_eq!(s.documento().blocos.len(), 2);
        assert_eq!(s.documento().blocos[1].atrib.alinhamento, crate::modelo::Alinhamento::Centro);
        s.desfazer();
        assert_eq!(s.documento().blocos.len(), 1);
    }

    #[test]
    fn justificado_enche_a_linha_menos_a_ultima() {
        let mut s = doc(&[&"palavra curta ".repeat(30)]);
        s.alinhar("justificado").unwrap();
        let lay = &s.layouts[0];
        assert!(lay.linhas.len() > 2);
        let util = s.documento().pagina.largura_util();
        let l = &lay.linhas[0];
        let u = l.pedacos.last().unwrap();
        let fim = u.x + crate::medida::largura(&u.texto, u.estilo, u.tamanho);
        assert!((fim - util).abs() < 0.5, "primeira linha vai até a margem direita: {fim} vs {util}");
        let ult = lay.linhas.last().unwrap().pedacos.last().unwrap();
        assert!(ult.x + crate::medida::largura(&ult.texto, ult.estilo, ult.tamanho) < util - 5.0);
    }

    #[test]
    fn recuar_paragrafo_e_item() {
        let mut s = doc(&["abc"]);
        s.recuar(1);
        assert!((s.documento().blocos[0].atrib.recuo_esq - PASSO_RECUO).abs() < 0.01);
        assert!((s.layouts[0].linhas[0].pedacos[0].x - PASSO_RECUO).abs() < 0.01);
        s.recuar(-1);
        assert_eq!(s.documento().blocos[0].atrib.recuo_esq, 0.0);
        assert!(s.alinhar("xis").is_err());
    }
}
