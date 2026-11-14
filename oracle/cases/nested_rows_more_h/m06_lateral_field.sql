select x.s from cc, lateral (select (c).street as s from replies c) x
