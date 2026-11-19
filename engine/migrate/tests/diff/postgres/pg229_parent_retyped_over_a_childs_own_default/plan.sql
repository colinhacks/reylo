alter table only c alter column c drop default;
alter table p alter column c type text[] using nullif(array[c], array[null]::text[]);
alter table c alter column c set default '{}'::text[];
