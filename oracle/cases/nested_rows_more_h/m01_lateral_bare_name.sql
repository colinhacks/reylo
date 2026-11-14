select x.c from cc, lateral (select c from replies c) x
