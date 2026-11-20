create type tag;
create function tag_in(cstring) returns tag language internal immutable strict as 'textin';
create function tag_out(tag) returns cstring language internal immutable strict as 'textout';
create type tag (internallength = variable, input = tag_in, output = tag_out, alignment = int4, storage = plain);
create cast (text as tag) without function as implicit;
