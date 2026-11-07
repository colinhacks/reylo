select x.c from holder2, lateral (select c from replies c) x
