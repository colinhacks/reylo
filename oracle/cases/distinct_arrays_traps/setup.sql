-- What a DISTINCT aggregate keeps distinct that is not: a SQL NULL and the document `null`, both `null` inside JSON; a typed empty array under coalesce, the type both take, whose cast can make two elements one; a type with a cast of its own to JSON, which `to_json` writes its values with; and a domain over an array, whose values nest.
create type mood as enum ('happy', 'sad');
create function mood_json(mood) returns json language sql immutable as $$ select '"same"'::json $$;
create cast (mood as json) with function mood_json(mood);
create domain intarr as int[];
create table j (id int primary key, v jsonb, m mood, i int8, n numeric, a intarr);
insert into j values (1, null, 'happy', 9007199254740992, 0.1, '{1,1}'), (2, 'null', 'sad', 9007199254740993, 0.10000000000000001, '{2,2}');
