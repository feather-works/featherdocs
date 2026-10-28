//! Inserir com parâmetros: tabela N×M, imagem, sumário com níveis, link, chamada (aviso,
//! dica…), e a contagem de palavras da aba Revisão.

use serde::Serialize;

use crate::alvo::{com_texto_ref, texto_alvo};
use crate::area::{recortar_blocos, texto_puro_do_bloco};
use crate::cursor::{Cursor, Pos};
use crate::digitacao::tem_texto_corrido;
use crate::formato::faixas;
use crate::modelo::{Conteudo, Trecho};
use crate::sessao::{Mudanca, Sessao, Splice};
use crate::texto::{fatia, modificar, plano};

/// A contagem completa (Revisão → Contagem de palavras).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Contagem {
    pub palavras: usize,
    pub caracteres: usize,
    pub caracteres_sem_espaco: usize,
    pub paragrafos: usize,
    pub linhas: usize,
    pub paginas: usize,
    /// A contagem é da seleção (e não do documento).
    pub selecao: bool,
}

/// Completa o endereço: e-mail vira `mailto:`, sem esquema ganha `https://`; `#` e caminhos ficam.
fn endereco(url: &str) -> Result<String, String> {
    let u = url.trim();
    if u.is_empty() {
        return Err("O endereço do link está vazio.".to_string());
    }
    if u.contains("://") || u.starts_with("mailto:") || u.starts_with('#') || u.starts_with('/') {
        return Ok(u.to_string());
    }
    if u.contains('@') && !u.contains(' ') {
        return Ok(format!("mailto:{u}"));
    }
    Ok(format!("https://{u}"))
}

/// As cores (fundo e barra) de cada tipo de chamada.
fn cores_da_chamada(tipo: &str) -> Result<Option<(u32, u32)>, String> {
    Ok(match tipo {
        "info" => Some((0xe8f0_feff, 0x1a73_e8ff)),
        "dica" => Some((0xe6f4_eaff, 0x1e8e_3eff)),
        "aviso" => Some((0xfef7_e0ff, 0xf9ab_00ff)),
        "atencao" => Some((0xfce8_e6ff, 0xd930_25ff)),
        "nenhuma" => None,
        _ => return Err(format!("Tipo de chamada desconhecido: \"{tipo}\".")),
    })
}

impl Sessao {
    /// Uma tabela de `linhas` × `colunas` (com linha de cabeçalho) no cursor.
    pub fn inserir_tabela(&mut self, linhas: usize, colunas: usize) -> Result<Mudanca, String> {
        if !(1..=200).contains(&linhas) || !(1..=30).contains(&colunas) {
            return Err("A tabela pode ter de 1 a 200 linhas e de 1 a 30 colunas.".to_string());
        }
        Ok(self.inserir_conteudo(Conteudo::Tabela { cabecalho: linhas > 1, linhas: vec![vec![Vec::new(); colunas]; linhas] }))
    }

    /// Uma imagem (`origem` = data URL ou caminho) com o tamanho natural em pontos (0 = automático).
    pub fn inserir_imagem(&mut self, origem: &str, largura: f32, altura: f32) -> Result<Mudanca, String> {
        if origem.trim().is_empty() {
            return Err("A imagem está vazia.".to_string());
        }
        let (l, a) = if largura > 0.0 && altura > 0.0 { (largura, altura) } else { (0.0, 0.0) };
        Ok(self.inserir_conteudo(Conteudo::Imagem { origem: origem.to_string(), legenda: Vec::new(), largura: l, altura: a }))
    }

    /// Troca apenas a fonte embutida de uma imagem existente, preservando ID,
    /// dimensões, legenda e uma única transação de undo/redo.
    /// Não interpreta SVG nem dados de gráfico: isso cabe ao adaptador IO.
    pub fn imagem_origem_atualizar(&mut self, id: u64, origem: &str) -> Result<Mudanca, String> {
        if origem.trim().is_empty() || origem.len() > 16 * 1024 * 1024 {
            return Err("A nova imagem está vazia ou excede 16 MiB.".into());
        }
        let indice = self.idx(id).ok_or("A imagem não existe mais.")?;
        let mut bloco = self.doc.blocos[indice].clone();
        let Conteudo::Imagem { origem: atual, .. } = &mut bloco.conteudo else {
            return Err("O bloco selecionado não é uma imagem.".into());
        };
        if atual == origem {
            return Ok(self.sem_mudanca());
        }
        *atual = origem.to_string();
        let cursor = self.cursor;
        Ok(self.transacao(Splice {
            pos: indice, quantos: 1, dentro: vec![bloco],
        }, cursor, None))
    }

    /// O texto alternativo da imagem `id` (o que o leitor de tela lê; vai para o `descr` do Word). Um Ctrl+Z.
    pub fn imagem_descricao_definir(&mut self, id: u64, descricao: &str) -> Result<Mudanca, String> {
        let indice = self.idx(id).ok_or("A imagem não existe mais.")?;
        let mut bloco = self.doc.blocos[indice].clone();
        if !matches!(bloco.conteudo, Conteudo::Imagem { .. }) {
            return Err("O bloco selecionado não é uma imagem.".into());
        }
        let descricao = descricao.trim();
        if bloco.imagem.descricao == descricao {
            return Ok(self.sem_mudanca());
        }
        bloco.imagem.descricao = descricao.to_string();
        let cursor = self.cursor;
        Ok(self.transacao(Splice { pos: indice, quantos: 1, dentro: vec![bloco] }, cursor, None))
    }

    /// Um sumário com os títulos de nível 1 até `niveis`.
    pub fn inserir_sumario(&mut self, niveis: u8) -> Mudanca {
        self.inserir_conteudo(Conteudo::sumario(niveis.clamp(1, 6)))
    }

    /// Sumário personalizado: os sumários que já existem passam a mostrar até `niveis`, com ou sem número
    /// de página e pontilhado; sem nenhum no documento, insere um no cursor.
    pub fn sumario_configurar(&mut self, niveis: u8, numeros: bool, pontilhado: bool) -> Mudanca {
        let novo = Conteudo::Sumario { niveis: niveis.clamp(1, 6), sem_numeros: !numeros, sem_pontilhado: !pontilhado, lista: 0, latex: false };
        let posicoes: Vec<usize> = self.doc.blocos.iter().enumerate().filter(|(_, b)| matches!(b.conteudo, Conteudo::Sumario { lista: 0, .. })).map(|(i, _)| i).collect();
        let (Some(&i0), Some(&i1)) = (posicoes.first(), posicoes.last()) else { return self.inserir_conteudo(novo) };
        let mut t = self.trabalho(i0, i1);
        let mut mudou = false;
        for b in t.blocos.iter_mut().filter(|b| matches!(b.conteudo, Conteudo::Sumario { lista: 0, .. })) {
            mudou |= b.conteudo != novo;
            b.conteudo = novo.clone();
        }
        if !mudou {
            return self.sem_mudanca();
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// Referências → Inserir legenda (`figura`, `tabela` ou `equacao`). Num parágrafo, ele vira a legenda
    /// (de novo, deixa de ser); numa imagem, tabela ou equação, entra um parágrafo de legenda centralizado
    /// junto dela — embaixo da figura e da equação, em cima da tabela (como a ABNT pede).
    pub fn legenda_inserir(&mut self, tipo: &str) -> Result<Mudanca, String> {
        let k: u8 = match tipo {
            "figura" => 1,
            "tabela" => 2,
            "equacao" => 3,
            outro => return Err(format!("Não conheço a legenda \"{outro}\".")),
        };
        let i = self.idx(self.cursor.foco.bloco).ok_or("O cursor está fora do documento.")?;
        if matches!(self.doc.blocos[i].conteudo, Conteudo::Paragrafo { .. }) {
            let mut t = self.trabalho(i, i);
            let a = &mut t.blocos[0].atrib;
            a.legenda = if a.legenda == k { 0 } else { k };
            if a.legenda != 0 {
                a.alinhamento = crate::modelo::Alinhamento::Centro;
            }
            let cursor = self.cursor;
            return Ok(self.fechar_trabalho(t, cursor, None));
        }
        let mut b = self.doc.novo_bloco(Conteudo::Paragrafo { texto: Vec::new() });
        b.atrib.legenda = k;
        b.atrib.alinhamento = crate::modelo::Alinhamento::Centro;
        let pos = if k == 2 { i } else { i + 1 };
        let cursor = Cursor::em(Pos { bloco: b.id, celula: None, desl: 0 });
        Ok(self.transacao(Splice { pos, quantos: 0, dentro: vec![b] }, cursor, None))
    }

    /// Referências → Lista de figuras (1) ou de tabelas (2), no cursor.
    pub fn lista_inserir(&mut self, lista: u8) -> Mudanca {
        self.inserir_conteudo(Conteudo::Sumario { niveis: 1, sem_numeros: false, sem_pontilhado: false, lista: lista.clamp(1, 2), latex: false })
    }

    /// Tira todos os sumários do documento.
    pub fn remover_sumario(&mut self) -> Mudanca {
        if !self.doc.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::Sumario { .. })) {
            return self.sem_mudanca();
        }
        let mut t = self.trabalho(0, self.doc.blocos.len() - 1);
        t.blocos.retain(|b| !matches!(b.conteudo, Conteudo::Sumario { .. }));
        if t.blocos.is_empty() {
            t.blocos.push(self.doc.novo_bloco(Conteudo::Paragrafo { texto: Vec::new() }));
        }
        let cursor = Cursor::em(Pos { bloco: t.blocos[0].id, celula: None, desl: 0 });
        self.fechar_trabalho(t, cursor, None)
    }

    /// Liga a seleção a `url`. Sem seleção, insere `texto` (ou o próprio endereço) já com o link.
    pub fn link_definir(&mut self, url: &str, texto: &str) -> Result<Mudanca, String> {
        let url = endereco(url)?;
        if self.cursor.vazio() {
            let mostrar = if texto.trim().is_empty() { url.trim_start_matches("mailto:").to_string() } else { texto.to_string() };
            return Ok(self.inserir_com_link(&mostrar, &url));
        }
        Ok(self.mudar_link(Some(url)))
    }

    /// Tira o link da seleção; sem seleção, do link inteiro onde está o cursor.
    pub fn link_remover(&mut self) -> Mudanca {
        if self.cursor.vazio() {
            self.selecionar_link_do_cursor();
        }
        self.mudar_link(None)
    }

    /// O link no cursor (para a caixa de link vir preenchida).
    pub fn link_no_cursor(&self) -> Option<String> {
        let f = self.cursor.foco;
        let t = self.idx(f.bloco).and_then(|i| texto_alvo(&self.doc.blocos[i].conteudo, f.celula))?;
        let ao_redor = |a: usize| fatia(&t, a, a + 1).into_iter().next().and_then(|x| x.link);
        ao_redor(f.desl.saturating_sub(1)).or_else(|| ao_redor(f.desl))
    }

    fn mudar_link(&mut self, link: Option<String>) -> Mudanca {
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        for p in faixas(&t, ini, fim) {
            com_texto_ref(&mut t.blocos[p.bloco].conteudo, p.celula, |x| modificar(x, p.a, p.b, |tr| tr.link = link.clone()));
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    pub(crate) fn inserir_com_link(&mut self, texto: &str, url: &str) -> Mudanca {
        let f = self.cursor.foco;
        let Some(i) = self.idx(f.bloco) else { return self.sem_mudanca() };
        let mut b = self.doc.blocos[i].clone();
        let n = texto.chars().count();
        let (estilo, fmt) = self.marca_no_cursor();
        let ok = com_texto_ref(&mut b.conteudo, f.celula, |x| {
            let (antes, depois) = crate::texto::dividir(x, f.desl);
            let meio = vec![Trecho { texto: texto.to_string(), estilo, link: Some(url.to_string()), fmt, ..Trecho::default() }];
            *x = crate::texto::juntar(crate::texto::juntar(antes, meio), depois);
        });
        if ok.is_none() {
            return self.sem_mudanca();
        }
        let cursor = Cursor::em(Pos { desl: f.desl + n, ..f });
        self.transacao(Splice { pos: i, quantos: 1, dentro: vec![b] }, cursor, None)
    }

    fn selecionar_link_do_cursor(&mut self) {
        let f = self.cursor.foco;
        let Some(t) = self.idx(f.bloco).and_then(|i| texto_alvo(&self.doc.blocos[i].conteudo, f.celula)) else { return };
        let Some(alvo) = self.link_no_cursor() else { return };
        let mut pos = 0;
        for tr in &t {
            let n = tr.texto.chars().count();
            if tr.link.as_deref() == Some(alvo.as_str()) && f.desl >= pos && f.desl <= pos + n {
                self.cursor.ancora = Pos { desl: pos, ..f };
                self.cursor.foco = Pos { desl: pos + n, ..f };
                return;
            }
            pos += n;
        }
    }

    /// Chamada (callout): `info`, `dica`, `aviso`, `atencao` dão fundo colorido e barra à esquerda; `nenhuma` tira.
    pub fn chamada(&mut self, tipo: &str) -> Result<Mudanca, String> {
        let cores = cores_da_chamada(tipo)?;
        Ok(self.mudar_atributos(|a| match cores {
            Some((fundo, barra)) => {
                a.fundo = fundo;
                a.borda = 1;
                a.cor_borda = barra;
            }
            None => {
                a.fundo = 0;
                a.borda = 0;
                a.cor_borda = 0;
            }
        }))
    }

    /// Palavras, caracteres, parágrafos e linhas: da seleção, se houver; senão do documento.
    pub fn contagem(&self) -> Contagem {
        let selecao = !self.cursor.vazio();
        let (blocos, linhas): (Vec<Conteudo>, usize) = if selecao {
            let (t, ini, fim) = self.trabalho_da_selecao();
            let linhas = (t.pos..t.pos + t.quantos).filter(|&i| com_texto(&self.doc.blocos[i].conteudo)).map(|i| self.layouts[i].linhas.len()).sum();
            (recortar_blocos(&t.blocos, ini, fim), linhas)
        } else {
            let linhas = self.doc.blocos.iter().zip(&self.layouts).filter(|(b, _)| com_texto(&b.conteudo)).map(|(_, l)| l.linhas.len()).sum();
            (self.doc.blocos.iter().map(|b| b.conteudo.clone()).collect(), linhas)
        };
        let textos: Vec<String> = blocos.iter().map(texto_puro_do_bloco).collect();
        let tudo = textos.join("\n");
        Contagem {
            palavras: tudo.split_whitespace().count(),
            // Tabulação e quebra são separadores de célula e de parágrafo, não caracteres digitados.
            caracteres: tudo.chars().filter(|c| *c != '\n' && *c != '\t').count(),
            caracteres_sem_espaco: tudo.chars().filter(|c| !c.is_whitespace()).count(),
            paragrafos: textos.iter().filter(|t| !t.trim().is_empty()).count(),
            linhas,
            paginas: self.paginas.len(),
            selecao,
        }
    }
}

/// Parágrafo (ou título, item, citação) com algum texto: o que conta linha na contagem.
fn com_texto(c: &Conteudo) -> bool {
    tem_texto_corrido(c) && !texto_de(c).trim().is_empty()
}

/// Para testes e para a interface: o texto puro de um conteúdo.
pub fn texto_de(c: &Conteudo) -> String {
    texto_alvo(c, None).map(|t| plano(&t)).unwrap_or_default()
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
    fn tabela_de_qualquer_tamanho() {
        let mut s = doc(&[""]);
        s.inserir_tabela(4, 6).unwrap();
        let Conteudo::Tabela { linhas, cabecalho } = &s.documento().blocos[0].conteudo else { panic!() };
        assert_eq!((linhas.len(), linhas[0].len(), *cabecalho), (4, 6, true));
        assert!(s.inserir_tabela(0, 3).is_err());
        assert!(s.inserir_tabela(3, 31).is_err());
    }

    #[test]
    fn sumario_personalizado_muda_o_que_existe_ou_insere() {
        let mut s = Sessao::default();
        s.sumario_configurar(2, false, false);
        let achar = |s: &Sessao| s.documento().blocos.iter().filter(|b| matches!(b.conteudo, Conteudo::Sumario { .. })).cloned().collect::<Vec<_>>();
        assert_eq!(achar(&s).len(), 1);
        s.sumario_configurar(4, true, false);
        let v = achar(&s);
        assert_eq!(v.len(), 1, "não insere outro");
        assert_eq!(v[0].conteudo, Conteudo::Sumario { niveis: 4, sem_numeros: false, sem_pontilhado: true, lista: 0, latex: false });
    }

    #[test]
    fn legendas_numeram_sozinhas_e_entram_na_lista() {
        let mut s = Sessao::default();
        s.inserir_imagem("a.png", 100.0, 50.0).unwrap();
        s.ir_para_bloco(s.documento().blocos[0].id);
        s.legenda_inserir("figura").unwrap();
        s.inserir_texto("Planta baixa");
        let i = s.documento().blocos.iter().position(|b| b.atrib.legenda == 1).unwrap();
        assert_eq!(i, 1, "a legenda da figura vem embaixo dela");
        assert_eq!(s.layouts[i].numero.as_ref().unwrap().texto, "Figura 1 –");
        s.inserir_tabela(2, 2).unwrap();
        s.legenda_inserir("tabela").unwrap();
        let t = s.documento().blocos.iter().position(|b| b.atrib.legenda == 2).unwrap();
        assert!(matches!(s.documento().blocos[t + 1].conteudo, Conteudo::Tabela { .. }), "a da tabela vem em cima");
        s.ir_para_bloco(s.documento().blocos[0].id);
        s.lista_inserir(1);
        let l = s.documento().blocos.iter().position(|b| matches!(b.conteudo, Conteudo::Sumario { lista: 1, .. })).unwrap();
        let textos: Vec<String> = s.layouts[l].linhas.iter().flat_map(|x| x.pedacos.iter().map(|p| p.texto.clone())).collect();
        assert!(textos.iter().any(|x| x == "Figura 1 – Planta baixa"), "{textos:?}");
        assert!(textos.iter().any(|x| x == "Lista de figuras"));
    }

    #[test]
    fn origem_de_imagem_atualiza_sem_perder_id_e_com_undo() {
        let mut s = Sessao::default();
        s.inserir_imagem("data:image/svg+xml;base64,AAAA", 100.0, 80.0).unwrap();
        let bloco = s.documento().blocos.iter()
            .find(|b| matches!(b.conteudo, Conteudo::Imagem { .. })).unwrap().clone();
        assert!(s.imagem_origem_atualizar(100_000, "data:image/svg+xml;base64,BBBB").is_err());
        assert!(s.imagem_origem_atualizar(bloco.id, "").is_err());
        let inicio = s.versao();
        s.imagem_origem_atualizar(bloco.id, "data:image/svg+xml;base64,BBBB").unwrap();
        assert!(s.versao() > inicio);
        let editado = s.documento().blocos.iter().find(|b| b.id == bloco.id).unwrap();
        assert!(matches!(&editado.conteudo,
            Conteudo::Imagem { origem, largura, altura, .. }
            if origem.ends_with("BBBB") && *largura == 100.0 && *altura == 80.0));
        s.desfazer().unwrap();
        assert_eq!(s.documento().blocos.iter().find(|b| b.id == bloco.id).unwrap(), &bloco);
        s.refazer().unwrap();
        let atual = s.documento().blocos.iter().find(|b| b.id == bloco.id).unwrap();
        assert!(matches!(&atual.conteudo, Conteudo::Imagem { origem, .. } if origem.ends_with("BBBB")));
        let v = s.versao();
        s.imagem_origem_atualizar(bloco.id, "data:image/svg+xml;base64,BBBB").unwrap();
        assert_eq!(s.versao(), v, "imagem idêntica não cria passo de undo");
    }

    #[test]
    fn imagem_e_sumario_e_remover_sumario() {
        let mut s = doc(&["texto"]);
        s.inserir_imagem("data:image/png;base64,AAAA", 200.0, 100.0).unwrap();
        assert!(s.documento().blocos.iter().any(|b| matches!(b.conteudo, Conteudo::Imagem { .. })));
        assert!(s.inserir_imagem(" ", 0.0, 0.0).is_err());
        s.inserir_sumario(2);
        assert!(s.documento().blocos.iter().any(|b| b.conteudo == Conteudo::sumario(2)));
        s.remover_sumario();
        assert!(!s.documento().blocos.iter().any(|b| matches!(b.conteudo, Conteudo::Sumario { .. })));
    }

    #[test]
    fn link_na_selecao_inserido_e_removido() {
        let mut s = doc(&["veja o site"]);
        s.mover(Movimento::FimDoDocumento, false);
        for _ in 0..4 {
            s.mover(Movimento::Esquerda, true);
        }
        s.link_definir("exemplo.com", "").unwrap();
        let Conteudo::Paragrafo { texto } = &s.documento().blocos[0].conteudo else { panic!() };
        assert_eq!(texto.last().unwrap().link.as_deref(), Some("https://exemplo.com"));
        s.mover(Movimento::FimDoDocumento, false);
        s.mover(Movimento::Esquerda, false);
        assert_eq!(s.link_no_cursor().as_deref(), Some("https://exemplo.com"));
        s.link_remover();
        let Conteudo::Paragrafo { texto } = &s.documento().blocos[0].conteudo else { panic!() };
        assert!(texto.iter().all(|t| t.link.is_none()));
        s.mover(Movimento::FimDoDocumento, false);
        s.link_definir("ana@x.org", "Ana").unwrap();
        assert_eq!(texto_de(&s.documento().blocos[0].conteudo), "veja o siteAna");
        assert_eq!(s.link_no_cursor().as_deref(), Some("mailto:ana@x.org"));
        assert!(s.link_definir("  ", "").is_err());
    }

    #[test]
    fn chamada_pinta_e_tira() {
        let mut s = doc(&["cuidado"]);
        s.chamada("aviso").unwrap();
        let a = s.documento().blocos[0].atrib;
        assert!(a.fundo != 0 && a.borda == 1 && a.cor_borda != 0);
        s.chamada("nenhuma").unwrap();
        assert_eq!(s.documento().blocos[0].atrib.fundo, 0);
        assert!(s.chamada("festa").is_err());
    }

    #[test]
    fn contagem_do_documento_e_da_selecao() {
        let mut s = doc(&["um dois três", "", "quatro"]);
        let c = s.contagem();
        assert_eq!((c.palavras, c.paragrafos, c.linhas, c.paginas, c.selecao), (4, 2, 2, 1, false));
        assert_eq!(c.caracteres_sem_espaco, 16);
        s.mover(Movimento::InicioDoDocumento, false);
        for _ in 0..2 {
            s.mover(Movimento::Direita, true);
        }
        let c = s.contagem();
        assert!(c.selecao && c.palavras == 1 && c.caracteres == 2);
    }
}
