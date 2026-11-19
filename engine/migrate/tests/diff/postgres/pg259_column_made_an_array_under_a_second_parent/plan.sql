alter table b alter column c type e[] using nullif(array[c], array[null]::e[]);
