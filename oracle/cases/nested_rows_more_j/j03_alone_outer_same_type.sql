select (select x.c from lateral (select c from replies c) x) as c from holder2
