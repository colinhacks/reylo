create table t (id integer primary key, s varchar, x integer, tags varchar[], meta struct("A" integer, b varchar),
        check (s in ('A', 'B')), check (regexp_matches(s, '^[A-Z]+$')), check (x between 1 and 10), check (len(tags) < 5),
        check (struct_extract(meta, 'A') > 0), check (case when x > 5 then s = 'B' else true end), check (x::varchar <> ''), check (coalesce(x, 0) >= 0),
        check (not (s is null)), check (list_contains(tags, 'X') = false), check (s similar to '[AB]'));
