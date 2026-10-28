//! Edição estrutural FeatherDocs via Tables, preservando trechos ricos.
//!
//! O motor neutro calcula índices e conteúdo textual. Cada trecho original
//! (links, Fmt, revisões, marcadores) acompanha sua célula ao mudar a grade.
//! A Sessao nativa aplica um único Splice: undo, paginação e FDOZ continuam
//! sob responsabilidade do FeatherDocs.

use featherdocs_core::modelo::{Conteudo, MesclagemTabela, Trecho};
use featherdocs_core::sessao::{Mudanca, Sessao};
use feather_tables::{Regiao, Tabela};

use crate::{comum_para_docs, docs_para_comum};

const MAX_LINHAS: usize = 200;
const MAX_COLUNAS: usize = 30;

#[derive(Clone, Copy)]
enum Eixo {
    Linha,
    Coluna,
}

/// Mapeia cada posição após inserção/exclusão para a posição de origem.
/// A inserção cria uma célula realmente vazia, sem herdar links/formatos.
fn indice_origem(atual: usize, indice: usize, inserir: bool) -> Option<usize> {
    if inserir {
        if atual == indice {
            None
        } else if atual > indice {
            Some(atual - 1)
        } else {
            Some(atual)
        }
    } else {
        Some(if atual >= indice { atual + 1 } else { atual })
    }
}

fn reordenar_trechos(
    original: &Conteudo,
    tabela: &Tabela,
    eixo: Eixo,
    indice: usize,
    inserir: bool,
) -> Result<Conteudo, String> {
    let Conteudo::Tabela { linhas, cabecalho } = original else {
        return Err("O bloco não é uma tabela.".into());
    };
    let (altura, largura) = tabela.dimensoes();
    let mut saida: Vec<Vec<Vec<Trecho>>> = Vec::with_capacity(altura);
    for linha in 0..altura {
        let mut nova = Vec::with_capacity(largura);
        for coluna in 0..largura {
            let r = match eixo {
                Eixo::Linha => indice_origem(linha, indice, inserir),
                Eixo::Coluna => Some(linha),
            };
            let c = match eixo {
                Eixo::Coluna => indice_origem(coluna, indice, inserir),
                Eixo::Linha => Some(coluna),
            };
            let trechos = r.zip(c)
                .and_then(|(r, c)| linhas.get(r).and_then(|l| l.get(c)))
                .cloned().unwrap_or_default();
            nova.push(trechos);
        }
        saida.push(nova);
    }
    let resultado = Conteudo::Tabela { cabecalho: *cabecalho, linhas: saida };
    // O modelo comum é a referência para dimensões e valores; não aplicar a
    // mudança se o mapeamento rico produziu uma grade diferente da calculada.
    let conferida = docs_para_comum(&resultado)
        .map_err(|e| format!("Tabela remapeada inválida: {e:?}"))?;
    if &conferida != tabela {
        return Err("A migração alteraria texto ou estrutura da tabela.".into());
    }
    Ok(resultado)
}

fn contexto(sessao: &Sessao) -> Result<(u64, usize, usize, Conteudo), String> {
    let pos = sessao.cursor().foco;
    let (linha, coluna) = pos.celula.ok_or("Posicione o cursor numa célula de tabela.")?;
    let conteudo = sessao.documento().blocos.iter()
        .find(|b| b.id == pos.bloco)
        .ok_or("O bloco do cursor não existe.")?
        .conteudo.clone();
    if !matches!(conteudo, Conteudo::Tabela { .. }) {
        return Err("Posicione o cursor numa célula de tabela.".into());
    }
    Ok((pos.bloco, linha as usize, coluna as usize, conteudo))
}

/// Executa linha+/linha-/coluna+/coluna- relativamente ao cursor atual.
/// Inserir ocorre após a célula; excluir afeta a célula corrente, conforme
/// os comandos anteriores do FeatherDocs. Não desloca estilos de uma
/// coluna/linha para outra: trechos completos são movidos por identidade.
pub fn docs_editar_estrutura(sessao: &mut Sessao, acao: &str) -> Result<Mudanca, String> {
    let (id, linha, coluna, origem) = contexto(sessao)?;
    if sessao.documento().blocos.iter().any(|b| b.id == id && (!b.mesclagens.is_empty() || b.tabela.tem_mescla_vertical())) {
        return Err("Desmescle as células antes de inserir ou excluir linhas/colunas.".into());
    }
    let mut tabela = docs_para_comum(&origem)
        .map_err(|e| format!("Tabela original inválida: {e:?}"))?;
    let (altura, largura) = tabela.dimensoes();
    if altura == 0 || largura == 0 || linha >= altura || coluna >= largura {
        return Err("A célula do cursor está fora da tabela.".into());
    }
    let (eixo, indice, inserir) = match acao {
        "linha+" => (Eixo::Linha, linha + 1, true),
        "linha-" => (Eixo::Linha, linha, false),
        "coluna+" => (Eixo::Coluna, coluna + 1, true),
        "coluna-" => (Eixo::Coluna, coluna, false),
        _ => return Err("Ação de tabela desconhecida.".into()),
    };
    if !inserir && (matches!(eixo, Eixo::Linha) && altura == 1
        || matches!(eixo, Eixo::Coluna) && largura == 1)
    {
        // Compatibilidade: última linha/coluna elimina a tabela e deixa
        // um parágrafo; a própria Sessao faz undo/repaginação desse caso.
        return sessao.tabela_excluir(if matches!(eixo, Eixo::Linha) {
            "linha"
        } else {
            "coluna"
        });
    }
    if inserir && (matches!(eixo, Eixo::Linha) && altura >= MAX_LINHAS
        || matches!(eixo, Eixo::Coluna) && largura >= MAX_COLUNAS)
    {
        return Err("A tabela atingiu o limite de 200 linhas ou 30 colunas.".into());
    }
    let operacao = match (eixo, inserir) {
        (Eixo::Linha, true) => tabela.inserir_linhas(indice, 1),
        (Eixo::Linha, false) => tabela.excluir_linhas(indice, 1),
        (Eixo::Coluna, true) => tabela.inserir_colunas(indice, 1),
        (Eixo::Coluna, false) => tabela.excluir_colunas(indice, 1),
    };
    operacao.map_err(|e| format!("Não foi possível alterar a tabela: {e:?}"))?;
    let atualizado = reordenar_trechos(&origem, &tabela, eixo, indice, inserir)?;
    let destino = match eixo {
        Eixo::Linha => (if inserir { indice } else { linha }, coluna),
        Eixo::Coluna => (linha, if inserir { indice } else { coluna }),
    };
    sessao.tabela_conteudo_substituir_com(id, atualizado, destino, |p| match (eixo, inserir) {
        (Eixo::Linha, true) => p.inserir_linha(indice),
        (Eixo::Linha, false) => p.remover_linha(indice),
        (Eixo::Coluna, true) => p.inserir_coluna(indice),
        (Eixo::Coluna, false) => p.remover_coluna(indice),
    })
}

/// Edita célula textual simples pelo motor comum; células com formatação
/// segmentada, objetos e revisões que não podem ser remapeados são recusadas
/// pelo conversor em vez de perder seus atributos.
pub fn docs_editar_celula(
    sessao: &mut Sessao,
    id: u64,
    linha: usize,
    coluna: usize,
    texto: &str,
) -> Result<Mudanca, String> {
    let origem = sessao.documento().blocos.iter()
        .find(|b| b.id == id)
        .ok_or("O bloco não existe.")?
        .conteudo.clone();
    let mut tabela = docs_para_comum(&origem)
        .map_err(|e| format!("Tabela original inválida: {e:?}"))?;
    tabela.atualizar_valor(
        linha, coluna, feather_tables::Valor::Texto(texto.into())
    ).map_err(|e| format!("Célula inválida: {e:?}"))?;
    let novo = comum_para_docs(&tabela, &origem)
        .map_err(|e| format!("Não posso alterar o texto rico sem perda: {e:?}"))?;
    sessao.tabela_conteudo_substituir(id, novo, (linha, coluna))
}


/// Mescla as células de uma seleção horizontal; com o cursor sem seleção,
/// mescla a célula atual com a seguinte à direita. Não apaga conteúdos:
/// o motor Tables recusa qualquer célula coberta que contenha valor.
pub fn docs_mesclar_celulas(sessao: &mut Sessao) -> Result<Mudanca, String> {
    let (id, linha, coluna, origem) = contexto(sessao)?;
    let cursor = sessao.cursor();
    let (mut inicio, mut fim) = match (cursor.ancora.bloco == id, cursor.ancora.celula) {
        (true, Some((l, c))) if l as usize == linha && c as usize != coluna =>
            ((c as usize).min(coluna), (c as usize).max(coluna)),
        _ => (coluna, coluna.checked_add(1).ok_or("Índice da célula inválido.")?),
    };
    let mut tabela = docs_para_comum(&origem)
        .map_err(|e| format!("Tabela original inválida: {e:?}"))?;
    let (altura, largura) = tabela.dimensoes();
    if linha >= altura || fim >= largura {
        return Err("Não há outra célula à direita para mesclar.".into());
    }
    let mut mesclagens = sessao.documento().blocos.iter()
        .find(|b| b.id == id).ok_or("Bloco de tabela inexistente.")?
        .mesclagens.clone();
    // Repetir "Mesclar →" amplia a mesclagem já selecionada à direita,
    // sem introduzir regiões sobrepostas nem descartar o conteúdo anterior.
    if cursor.ancora.celula == cursor.foco.celula {
        if let Some(pos) = mesclagens.iter().position(|m| m.linha == linha && m.coluna == coluna) {
            let atual = mesclagens.remove(pos);
            inicio = atual.coluna;
            fim = atual.coluna.checked_add(atual.colunas)
                .ok_or("A região mesclada é inválida.")?;
        }
    }
    if fim >= largura {
        return Err("Não há outra célula à direita para mesclar.".into());
    }
    for m in &mesclagens {
        tabela.mesclar(Regiao {
            linha: m.linha, coluna: m.coluna, linhas: 1, colunas: m.colunas,
        }).map_err(|e| format!("Mesclagem existente inválida: {e:?}"))?;
    }
    tabela.mesclar(Regiao {
        linha, coluna: inicio, linhas: 1, colunas: fim - inicio + 1,
    }).map_err(|e| format!("Não é possível mesclar células preenchidas ou sobrepostas: {e:?}"))?;
    mesclagens.push(MesclagemTabela { linha, coluna: inicio, colunas: fim - inicio + 1 });
    mesclagens.sort_by_key(|m| (m.linha, m.coluna));
    sessao.tabela_mesclagens_definir(id, mesclagens, (linha, inicio))
}

/// Desmescla a região em que está o cursor, conservando os trechos intactos.
pub fn docs_desmesclar_celulas(sessao: &mut Sessao) -> Result<Mudanca, String> {
    let (id, linha, coluna, _) = contexto(sessao)?;
    let mut mesclagens = sessao.documento().blocos.iter()
        .find(|b| b.id == id).ok_or("Bloco de tabela inexistente.")?
        .mesclagens.clone();
    let before = mesclagens.len();
    mesclagens.retain(|m| !m.contem(linha, coluna));
    if mesclagens.len() == before {
        return Err("A célula selecionada não está mesclada.".into());
    }
    sessao.tabela_mesclagens_definir(id, mesclagens, (linha, coluna))
}
