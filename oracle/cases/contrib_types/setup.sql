-- The contrib modules' types that Supabase and Neon ship: cube, earthdistance's earth (a domain
-- over cube), seg, isn's numbers, and lo (a domain over oid), with oid itself. None has a cast
-- to JSON, so inside a JSON document each is written as its text, oid and lo included.
create extension cube;
create extension earthdistance;
create extension seg;
create extension isn;
create extension lo;
create table stock (
  id int primary key,
  dims cube not null,
  spot earth,
  span seg,
  isbn isbn,
  isbn13 isbn13 not null,
  ean ean13,
  issn issn,
  ismn ismn,
  upc upc,
  blob lo,
  ref oid,
  boxes cube[]
);
