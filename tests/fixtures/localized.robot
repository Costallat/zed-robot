Language: Brazilian Portuguese

*** Configurações ***
Documentação    Exemplo
Biblioteca    Collections

*** Variáveis ***
${NOME}    mundo

*** Casos de Teste ***
Teste
    [Documentação]    Exemplo
    [Etiquetas]    smoke
    Log    olá ${NOME}
    Dizer Olá

*** Palavras-Chave ***
Dizer Olá
    [Argumentos]    ${quem}=${NOME}
    Log    olá ${quem}
