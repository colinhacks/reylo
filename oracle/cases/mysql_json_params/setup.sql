-- A JSON parameter is bound as its JSON text, which the server reads as a document where it is
-- written whole to a column, cast to JSON, or the document a JSON function reads. Anywhere else
-- (a value a function writes, the value of a MEMBER OF, a comparison) the text would be a string,
-- stored or compared with its quotes, so the statement is refused until the parameter is cast.
create table profiles (id int primary key, doc json not null);
