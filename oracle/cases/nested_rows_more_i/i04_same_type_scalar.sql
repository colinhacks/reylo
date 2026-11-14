select x.one from holder2, lateral (select (select c from replies c limit 1) as one) x
