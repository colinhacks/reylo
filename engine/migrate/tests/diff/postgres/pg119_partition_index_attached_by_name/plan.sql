create index ti on only t using btree (i);
alter index ti attach partition pz;
