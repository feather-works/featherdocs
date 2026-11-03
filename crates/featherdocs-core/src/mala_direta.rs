//! Mala direta: o documento com campos `{{nome}}` vira uma cópia por linha de uma planilha (a
//! primeira linha dá os nomes dos campos), separadas por quebra de página.
use crate::busca::Opcoes;
use crate::modelo::{Conteudo, Documento};
use crate::sessao::Sessao;

/// O documento de todas as cartas: uma cópia de `modelo` por linha de `tabela` (sem o cabeçalho),
/// com cada `{{campo}}` trocado pelo valor da coluna de mesmo nome (sem diferenciar maiúsculas).
// shortcut: compõe uma Sessao por linha (reaproveita o "substituir tudo"); serve a centenas de linhas, não a dezenas de milhares.
pub fn gerar(modelo: &Documento, tabela: &[Vec<String>]) -> Result<Documento, String> {
    let (cabecalho, linhas) = tabela.split_first().ok_or("A planilha está vazia.")?;
    let texto: String = modelo.blocos.iter().flat_map(|b| b.conteudo.textos()).map(|t| crate::texto::plano(t)).collect::<Vec<_>>().join("\n").to_lowercase();
    let campos: Vec<(usize, String)> = cabecalho
        .iter()
        .enumerate()
        .map(|(i, c)| (i, format!("{{{{{}}}}}", c.trim())))
        .filter(|(_, c)| c.len() > 4 && texto.contains(&c.to_lowercase()))
        .collect();
    if campos.is_empty() {
        return Err("O documento não tem campos com os nomes da primeira linha da planilha. Escreva-os entre chaves duplas, como {{nome}}.".into());
    }
    let linhas: Vec<&Vec<String>> = linhas.iter().filter(|l| l.iter().any(|c| !c.trim().is_empty())).collect();
    if linhas.is_empty() {
        return Err("A planilha só tem a linha dos nomes dos campos.".into());
    }
    let mut saida = modelo.clone();
    saida.blocos.clear();
    for (k, linha) in linhas.into_iter().enumerate() {
        let mut s = Sessao::com_documento(modelo.clone());
        for (i, campo) in &campos {
            if s.busca_definir(campo, Opcoes::default()).erro.is_none() {
                s.substituir_tudo(linha.get(*i).map_or("", |v| v.trim()));
            }
        }
        if k > 0 {
            let quebra = saida.novo_bloco(Conteudo::QuebraDePagina);
            saida.blocos.push(quebra);
        }
        for b in &s.documento().blocos {
            let novo = saida.novo_bloco(b.conteudo.clone());
            saida.blocos.push(crate::modelo::Bloco { id: novo.id, ..b.clone() });
        }
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::Trecho;

    fn tabela(linhas: &[&[&str]]) -> Vec<Vec<String>> {
        linhas.iter().map(|l| l.iter().map(|c| c.to_string()).collect()).collect()
    }

    #[test]
    fn uma_carta_por_linha_com_os_campos_trocados() {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("Prezado(a) {{Nome}}, de {{cidade}}.")] });
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("Atenciosamente.")] });
        let t = tabela(&[&["nome", "cidade", "sobra"], &["Ana", "Recife", "x"], &[" ", ""], &["Bia", "Natal"]]);
        let r = gerar(&d, &t).unwrap();
        let textos: Vec<String> = r.blocos.iter().map(|b| b.conteudo.textos().first().map_or("—".into(), |t| crate::texto::plano(t))).collect();
        assert_eq!(textos, ["Prezado(a) Ana, de Recife.", "Atenciosamente.", "—", "Prezado(a) Bia, de Natal.", "Atenciosamente."]);
        let mut ids: Vec<u64> = r.blocos.iter().map(|b| b.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), r.blocos.len(), "cada bloco com id próprio");
    }

    #[test]
    fn sem_campos_ou_sem_linhas_explica_o_que_falta() {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("Sem campos.")] });
        assert!(gerar(&d, &tabela(&[&["nome"], &["Ana"]])).unwrap_err().contains("{{nome}}"));
        assert!(gerar(&d, &[]).is_err());
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("{{nome}}")] });
        assert!(gerar(&d, &tabela(&[&["nome"]])).unwrap_err().contains("só tem"));
    }
}
