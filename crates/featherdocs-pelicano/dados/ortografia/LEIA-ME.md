# Dicionário de ortografia pt-BR

`pt_BR.zip` traz os arquivos `pt_BR.aff` e `pt_BR.dic` do **VERO – Verificador Ortográfico do
LibreOffice** (versão 3.2), de Raimundo Santos Moura e colaboradores, sem modificação, compactados.

- Origem: <https://github.com/LibreOffice/dictionaries/tree/master/pt_BR>
- Licença: LGPLv3 ou MPL (à escolha), ver `README_pt_BR.txt`.

O FeatherDocs lê o dicionário com o crate `spellbook` (MPL-2.0), tudo no computador, sem rede.

# Dicionário de sinônimos pt-BR

`sinonimos_pt_BR.zip` traz `sinonimos_pt_BR.txt`, **versão modificada** do `th_pt_BR.dat` (o
dicionário de sinônimos em português do Brasil do LibreOffice, formato MyThes).

- Origem: <https://github.com/LibreOffice/dictionaries/tree/master/pt_BR>
- Licença: a de Kevin B. Hendricks e colaboradores (2003), copiada sem mudança em
  `LICENCA-SINONIMOS.txt` (o `license-thes.readme` de lá).
- **O que mudou**: cada linha é `palavra|sinônimo|sinônimo|…`. Os grupos de sentido e os rótulos
  "(Sinônimo)" saíram, os sinônimos repetidos foram juntados e ficam no máximo 30 por palavra.
