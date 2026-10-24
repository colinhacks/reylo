/*
** SQLite's own resolved parse tree, as JSON, for the engine's SQLite analysis.
**
** Built as ONE translation unit with the patched amalgamation (csrc/patch-sqlite.py inserts six
** one-line hooks, five of them right after name resolution), so the SQLITE_PRIVATE internals are
** callable here. The engine gives this file the schema DDL and one statement; the hooks fire while
** SQLite prepares the statement against an in-memory replay of that schema, and the walker
** serializes what SQLite resolved: every expression with sqlite3ExprAffinity and
** sqlite3ExprCanBeNull, every column reference with its Table, Column, NOT NULL and outer-join
** mark (EP_CanBeNull), the FROM items with their join type and cursor, the DML target columns per
** value position, and each table's columns and unique keys. The rules (function result types,
** arithmetic, the row-count proof) live in Rust; nothing here decides a type. A host that
** registers scalar functions of its own on its connection names them with their argument counts,
** and the replay registers a stub for each, so a statement calling one prepares here as it does
** there. A host that lists the functions of its own SQLite (pragma_function_list) decides which
** functions exist: one it has and this build lacks gets a stub too, and one this build has and it
** lacks is refused.
*/
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

struct Parse; struct Select; struct SelectDest; struct Table; struct IdList; struct ExprList; struct Expr; struct Upsert; struct SrcList;
typedef void (*reylo_select_fn)(struct Parse*, struct Select*, struct SelectDest*);
typedef void (*reylo_insert_fn)(struct Parse*, struct Table*, struct IdList*, struct ExprList*, struct Select*, int, struct Upsert*);
typedef void (*reylo_update_fn)(struct Parse*, struct Table*, struct ExprList*, struct Expr*, int*, struct SrcList*, int, struct Upsert*);
typedef void (*reylo_delete_fn)(struct Parse*, struct Table*, struct Expr*);
typedef void (*reylo_returning_fn)(struct Parse*, struct Table*, struct ExprList*);
typedef void (*reylo_insert_select_fn)(struct Parse*, struct Select*);
static reylo_select_fn reylo_select_hook;
static reylo_insert_fn reylo_insert_hook;
static reylo_update_fn reylo_update_hook;
static reylo_delete_fn reylo_delete_hook;
static reylo_returning_fn reylo_returning_hook;
static reylo_insert_select_fn reylo_insert_select_hook;

#include "sqlite3-patched.c"

/* ---- capture state: one statement at a time ------------------------------------------------ */

static sqlite3 *gDb;            /* the in-memory schema replay */
static char *gDdlKey;           /* the DDL it was built from */
static sqlite3_str *gStmt;      /* the top-level SELECT, or a DML's SELECT source */
static sqlite3_str *gDml;       /* the INSERT / UPDATE / DELETE */
static sqlite3_str *gReturning; /* the RETURNING list */
static int gCaptured;           /* the first top-level hook has fired */
static Select *gInsertSelect;   /* the SELECT or VALUES that feeds the INSERT, about to be coded */
static Table *gTables[128];
static int gnTables;

/* A table of the schema the statement reads. A WITH query's table is ephemeral, as a subquery's
** is: the statement owns it and frees it as the prepare ends, before tables() reads the list. */
static void tableSeen(Table *pTab){
  int i;
  if( pTab==0 || IsView(pTab) || IsVirtual(pTab) || (pTab->tabFlags & TF_Ephemeral) ) return;
  for(i=0; i<gnTables; i++) if( gTables[i]==pTab ) return;
  if( gnTables<128 ) gTables[gnTables++] = pTab;
}

/* ---- JSON helpers -------------------------------------------------------------------------- */

static void js(sqlite3_str *s, const char *z){
  if( z==0 ){ sqlite3_str_appendall(s, "null"); return; }
  sqlite3_str_appendchar(s, 1, '"');
  for(; *z; z++){
    unsigned char c = (unsigned char)*z;
    switch( c ){
      case '"': sqlite3_str_appendall(s, "\\\""); break;
      case '\\': sqlite3_str_appendall(s, "\\\\"); break;
      case '\n': sqlite3_str_appendall(s, "\\n"); break;
      case '\r': sqlite3_str_appendall(s, "\\r"); break;
      case '\t': sqlite3_str_appendall(s, "\\t"); break;
      default:
        if( c<0x20 ) sqlite3_str_appendf(s, "\\u%04x", c);
        else sqlite3_str_appendchar(s, 1, (char)c);
    }
  }
  sqlite3_str_appendchar(s, 1, '"');
}
static void key(sqlite3_str *s, const char *k){ js(s, k); sqlite3_str_appendchar(s, 1, ':'); }
static void kv_str(sqlite3_str *s, const char *k, const char *v){ key(s, k); js(s, v); sqlite3_str_appendchar(s, 1, ','); }
static void kv_int(sqlite3_str *s, const char *k, int v){ key(s, k); sqlite3_str_appendf(s, "%d,", v); }
static void kv_bool(sqlite3_str *s, const char *k, int v){ key(s, k); sqlite3_str_appendall(s, v ? "true," : "false,"); }
#ifndef SQLITE_OMIT_WINDOWFUNC
/* A window frame's bound by the parser's token for it. */
static const char *frameBound(u8 e){
  switch( e ){
    case TK_UNBOUNDED: return "unbounded";
    case TK_CURRENT: return "current";
    case TK_PRECEDING: return "preceding";
    case TK_FOLLOWING: return "following";
    default: return "";
  }
}
#endif
/* Closes an object whose last member left a trailing comma. */
static void closeObj(sqlite3_str *s){
  int n = sqlite3_str_length(s);
  char *z = sqlite3_str_value(s);
  if( n>0 && z[n-1]==',' ) z[n-1] = '}'; else sqlite3_str_appendchar(s, 1, '}');
}
static void closeArr(sqlite3_str *s){
  int n = sqlite3_str_length(s);
  char *z = sqlite3_str_value(s);
  if( n>0 && z[n-1]==',' ) z[n-1] = ']'; else sqlite3_str_appendchar(s, 1, ']');
}

static const char *affName(char a){
  switch( a ){
    case SQLITE_AFF_BLOB: return "blob";
    case SQLITE_AFF_TEXT: return "text";
    case SQLITE_AFF_NUMERIC: return "numeric";
    case SQLITE_AFF_INTEGER: return "integer";
    case SQLITE_AFF_REAL: return "real";
    case SQLITE_AFF_FLEXNUM: return "numeric";
    default: return "none";
  }
}
static const char *opName(int op){
  switch( op ){
    case TK_COLUMN: case TK_AGG_COLUMN: case TK_TRIGGER: return "column";
    case TK_VARIABLE: return "variable"; case TK_INTEGER: return "integer"; case TK_FLOAT: return "float";
    case TK_STRING: return "string"; case TK_BLOB: return "blob"; case TK_NULL: return "null"; case TK_TRUEFALSE: return "truefalse";
    case TK_FUNCTION: case TK_AGG_FUNCTION: return "function";
    case TK_CAST: return "cast"; case TK_CONCAT: return "||"; case TK_PLUS: return "+"; case TK_MINUS: return "-"; case TK_STAR: return "*";
    case TK_SLASH: return "/"; case TK_REM: return "%"; case TK_EQ: return "="; case TK_NE: return "!="; case TK_LT: return "<"; case TK_LE: return "<=";
    case TK_GT: return ">"; case TK_GE: return ">="; case TK_IS: return "is"; case TK_ISNOT: return "is not"; case TK_IN: return "in"; case TK_BETWEEN: return "between";
    case TK_AND: return "and"; case TK_OR: return "or"; case TK_NOT: return "not"; case TK_BITAND: return "&"; case TK_BITOR: return "|";
    case TK_LSHIFT: return "<<"; case TK_RSHIFT: return ">>"; case TK_BITNOT: return "~"; case TK_UMINUS: return "neg"; case TK_UPLUS: return "pos";
    case TK_ISNULL: return "isnull"; case TK_NOTNULL: return "notnull"; case TK_SELECT: return "select"; case TK_EXISTS: return "exists";
    case TK_CASE: return "case"; case TK_COLLATE: return "collate"; case TK_VECTOR: return "vector"; case TK_SELECT_COLUMN: return "select_column";
    case TK_IF_NULL_ROW: return "if_null_row"; case TK_REGISTER: return "register"; case TK_ID: return "id"; case TK_ROW: return "row";
    default: return "other";
  }
}

static void selectJson(sqlite3_str *s, Select *p);
static void expr(sqlite3_str *s, Expr *p, Select *pSel);

static void exprList(sqlite3_str *s, ExprList *pList, Select *pSel){
  int i;
  sqlite3_str_appendchar(s, 1, '[');
  for(i=0; pList && i<pList->nExpr; i++){ expr(s, pList->a[i].pExpr, pSel); sqlite3_str_appendchar(s, 1, ','); }
  closeArr(s);
}

static int outerCursor(Select *pSel, int iCursor){
  int i;
  if( pSel==0 || pSel->pSrc==0 ) return 0;
  for(i=0; i<pSel->pSrc->nSrc; i++){
    SrcItem *pItem = &pSel->pSrc->a[i];
    if( pItem->iCursor==iCursor ) return (pItem->fg.jointype & (JT_LEFT|JT_LTORJ))!=0;
  }
  return 0;
}

/* The name a statement reads a rowid table's rowid by: "rowid", "_rowid_" or "oid", the first no
** declared column has, since a declared column of that name hides the rowid. 0 when all three do. */
static const char *rowidName(Table *pTab){
  static const char *azNames[] = { "rowid", "_rowid_", "oid" };
  int i, j;
  for(i=0; i<3; i++){
    for(j=0; j<pTab->nCol && sqlite3StrICmp(pTab->aCol[j].zCnName, azNames[i])!=0; j++){}
    if( j==pTab->nCol ) return azNames[i];
  }
  return 0;
}

/* The provenance members of a column reference (the object is already open). */
static void columnRef(sqlite3_str *s, Expr *p, Select *pSel){
  Table *pTab = ExprUseYTab(p) ? p->y.pTab : 0;
  int iCol = p->iColumn;
  kv_int(s, "cursor", p->iTable);
  kv_int(s, "index", iCol);
  if( pTab==0 ){ kv_bool(s, "resolved", 0); return; }
  tableSeen(pTab);
  kv_str(s, "table", pTab->zName);
  if( iCol<0 ) iCol = pTab->iPKey;
  if( iCol<0 ){
    kv_str(s, "column", rowidName(pTab) ? rowidName(pTab) : "rowid");
    kv_str(s, "affinity", "integer");
    kv_bool(s, "notNull", 1);
    kv_bool(s, "rowid", 1);
  }else{
    Column *pCol = &pTab->aCol[iCol];
    kv_str(s, "column", pCol->zCnName);
    kv_str(s, "declared", sqlite3ColumnType(pCol, ""));
    kv_str(s, "affinity", affName(pCol->affinity));
    /* The collation a comparison with the column uses, where the column decides it. */
    kv_str(s, "coll", sqlite3ColumnColl(pCol) ? sqlite3ColumnColl(pCol) : sqlite3StrBINARY);
    kv_bool(s, "notNull", pCol->notNull!=0);
    kv_bool(s, "rowid", p->iColumn<0 || iCol==pTab->iPKey);
    if( pCol->colFlags & COLFLAG_GENERATED ) kv_bool(s, "generated", 1);
  }
  kv_bool(s, "outer", ExprHasProperty(p, EP_CanBeNull) || (p->op==TK_COLUMN && outerCursor(pSel, p->iTable)));
  if( pTab->tabFlags & TF_Strict ) kv_bool(s, "strict", 1);
  if( IsView(pTab) ) kv_bool(s, "view", 1);
}

static void expr(sqlite3_str *s, Expr *p, Select *pSel){
  int op;
  if( p==0 ){ sqlite3_str_appendall(s, "null"); return; }
  op = p->op;
  if( op==TK_REGISTER ) op = p->op2;
  sqlite3_str_appendchar(s, 1, '{');
  kv_str(s, "op", opName(op));
  kv_str(s, "aff", affName(sqlite3ExprAffinity(p)));
  kv_bool(s, "canBeNull", sqlite3ExprCanBeNull(p));
  /* SQLite moves every ON term into the WHERE; one from an outer join's ON says so, and names the
  ** cursor of the join's right side, which the term restricts without removing a row. */
  if( ExprHasProperty(p, EP_OuterON) ){ kv_bool(s, "outerOn", 1); kv_int(s, "join", p->w.iJoin); }
  switch( op ){
    case TK_COLUMN: case TK_AGG_COLUMN: case TK_TRIGGER:
      columnRef(s, p, pSel);
      break;
    case TK_VARIABLE:
      if( ExprHasProperty(p, EP_IntValue) ){ key(s, "name"); sqlite3_str_appendf(s, "\"?%d\",", p->iColumn); }
      else kv_str(s, "name", p->u.zToken);
      kv_int(s, "index", p->iColumn);
      break;
    case TK_INTEGER:
      if( ExprHasProperty(p, EP_IntValue) ){ key(s, "value"); sqlite3_str_appendf(s, "\"%d\",", p->u.iValue); }
      else kv_str(s, "value", p->u.zToken);
      break;
    case TK_FLOAT: case TK_STRING: case TK_BLOB: case TK_TRUEFALSE: case TK_ID:
      kv_str(s, "value", ExprHasProperty(p, EP_IntValue) ? "" : p->u.zToken);
      break;
    case TK_FUNCTION: case TK_AGG_FUNCTION: {
      char *zName = sqlite3_mprintf("%s", ExprHasProperty(p, EP_IntValue) ? "" : p->u.zToken);
      char *z;
      for(z=zName; z && *z; z++) *z = (char)sqlite3Tolower(*z);
      kv_str(s, "name", zName);
      sqlite3_free(zName);
      kv_bool(s, "agg", p->op==TK_AGG_FUNCTION);
      kv_bool(s, "window", ExprHasProperty(p, EP_WinFunc));
      /* A FILTER alone is held as a window too; `over` is a call with an OVER clause. */
      kv_bool(s, "over", IsWindowFunc(p));
      kv_bool(s, "distinct", ExprHasProperty(p, EP_Distinct));
      kv_bool(s, "infix", ExprHasProperty(p, EP_InfixFunc));
      key(s, "args");
      if( !ExprHasProperty(p, EP_TokenOnly|EP_Reduced) && ExprUseXList(p) ) exprList(s, p->x.pList, pSel);
      else sqlite3_str_appendall(s, "[]");
      sqlite3_str_appendchar(s, 1, ',');
      /* An aggregate's own ORDER BY, and a window's PARTITION BY and ORDER BY (a named window's
      ** too, which resolving the call copied into it). */
#ifndef SQLITE_OMIT_WINDOWFUNC
      if( IsWindowFunc(p) && p->y.pWin ){
        Window *pWin = p->y.pWin;
        key(s, "partitionBy"); exprList(s, pWin->pPartition, pSel); sqlite3_str_appendchar(s, 1, ',');
        key(s, "orderBy"); exprList(s, pWin->pOrderBy, pSel); sqlite3_str_appendchar(s, 1, ',');
        /* The frame as resolving the call left it, a named window's copied in: where it starts and
        ** ends (a start `unbounded` is UNBOUNDED PRECEDING, an end UNBOUNDED FOLLOWING), what it
        ** excludes, and whether a FILTER narrows the rows it aggregates. */
        kv_str(s, "frameStart", frameBound(pWin->eStart));
        kv_str(s, "frameEnd", frameBound(pWin->eEnd));
        kv_str(s, "exclude", pWin->eExclude==TK_CURRENT ? "current" : pWin->eExclude==TK_GROUP ? "group" : pWin->eExclude==TK_TIES ? "ties" : "no");
        kv_bool(s, "filter", pWin->pFilter!=0);
      }else
#endif
      if( !ExprHasProperty(p, EP_TokenOnly|EP_Reduced) && p->pLeft && p->pLeft->op==TK_ORDER && ExprUseXList(p->pLeft) ){
        key(s, "orderBy"); exprList(s, p->pLeft->x.pList, pSel); sqlite3_str_appendchar(s, 1, ',');
      }
      break;
    }
    case TK_CAST:
      kv_str(s, "to", ExprHasProperty(p, EP_IntValue) ? "" : p->u.zToken);
      kv_str(s, "affinity", affName(sqlite3AffinityType(ExprHasProperty(p, EP_IntValue) ? "" : p->u.zToken, 0)));
      /* fall through: the operand is pLeft */
      key(s, "left"); expr(s, ExprHasProperty(p, EP_TokenOnly) ? 0 : p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ',');
      break;
    case TK_IN:
      key(s, "left"); expr(s, p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ',');
      if( ExprUseXSelect(p) ){ key(s, "select"); selectJson(s, p->x.pSelect); }
      else { key(s, "list"); exprList(s, p->x.pList, pSel); }
      sqlite3_str_appendchar(s, 1, ',');
      break;
    case TK_BETWEEN: case TK_CASE: case TK_VECTOR:
      key(s, "left"); expr(s, p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ',');
      key(s, "list"); if( ExprUseXList(p) ) exprList(s, p->x.pList, pSel); else sqlite3_str_appendall(s, "[]");
      sqlite3_str_appendchar(s, 1, ',');
      break;
    case TK_SELECT: case TK_EXISTS:
      key(s, "select"); if( ExprUseXSelect(p) ) selectJson(s, p->x.pSelect); else sqlite3_str_appendall(s, "null");
      sqlite3_str_appendchar(s, 1, ',');
      break;
    case TK_SELECT_COLUMN:
      kv_int(s, "index", p->iColumn);
      key(s, "left"); expr(s, p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ',');
      break;
    default:
      if( !ExprHasProperty(p, EP_TokenOnly) ){
        if( p->pLeft ){ key(s, "left"); expr(s, p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ','); }
        if( p->pRight ){ key(s, "right"); expr(s, p->pRight, pSel); sqlite3_str_appendchar(s, 1, ','); }
      }
      break;
  }
  closeObj(s);
}

static void selectJson(sqlite3_str *s, Select *pTop){
  Select *p = pTop;
  int i, nMore = 0;
  if( p==0 ){ sqlite3_str_appendall(s, "null"); return; }
  /* A multi-row VALUES is a chain of one-row SELECTs under UNION ALL, the last row on top. Its
  ** first row stands for it here, with the rows after it in "rows", so a list of any length nests
  ** nothing; an ORDER BY or a LIMIT is on the top row, as on any compound. */
  while( (p->selFlags & SF_Values) && p->op==TK_ALL && p->pPrior && (p->pPrior->selFlags & SF_Values) ){ p = p->pPrior; nMore++; }
  sqlite3_str_appendchar(s, 1, '{');
  key(s, "columns"); sqlite3_str_appendchar(s, 1, '[');
  for(i=0; p->pEList && i<p->pEList->nExpr; i++){
    struct ExprList_item *pItem = &p->pEList->a[i];
    sqlite3_str_appendchar(s, 1, '{');
    kv_str(s, "name", pItem->zEName);
    kv_bool(s, "named", pItem->fg.eEName==ENAME_NAME);
    key(s, "expr"); expr(s, pItem->pExpr, p); sqlite3_str_appendchar(s, 1, ',');
    closeObj(s); sqlite3_str_appendchar(s, 1, ',');
  }
  closeArr(s); sqlite3_str_appendchar(s, 1, ',');
  if( nMore ){
    Select **apRow = sqlite3_malloc64(sizeof(Select*)*(sqlite3_uint64)nMore);
    Select *q;
    key(s, "rows"); sqlite3_str_appendchar(s, 1, '[');
    for(i=nMore, q=pTop; apRow && i>0; q=q->pPrior) apRow[--i] = q;
    for(i=0; apRow && i<nMore; i++){ exprList(s, apRow[i]->pEList, apRow[i]); sqlite3_str_appendchar(s, 1, ','); }
    sqlite3_free(apRow);
    closeArr(s); sqlite3_str_appendchar(s, 1, ',');
  }
  key(s, "from"); sqlite3_str_appendchar(s, 1, '[');
  for(i=0; p->pSrc && i<p->pSrc->nSrc; i++){
    SrcItem *pItem = &p->pSrc->a[i];
    sqlite3_str_appendchar(s, 1, '{');
    kv_int(s, "cursor", pItem->iCursor);
    /* A table by the name its DDL spells, which "tables" is keyed by: SQLite's names fold ASCII
    ** case, so the statement may spell it another way. A table-valued function by its module's. */
    kv_str(s, "table", pItem->pSTab && !pItem->fg.isSubquery ? pItem->pSTab->zName : pItem->zName);
    kv_str(s, "alias", pItem->zAlias);
    kv_bool(s, "outer", (pItem->fg.jointype & (JT_LEFT|JT_LTORJ))!=0);
    kv_bool(s, "cross", (pItem->fg.jointype & JT_CROSS)!=0);
    kv_bool(s, "right", (pItem->fg.jointype & JT_RIGHT)!=0);
    kv_bool(s, "tabFunc", pItem->fg.isTabFunc);
    if( pItem->fg.isTabFunc ){ key(s, "args"); exprList(s, pItem->u1.pFuncArg, p); sqlite3_str_appendchar(s, 1, ','); }
    kv_bool(s, "cte", pItem->fg.isCte);
    /* A recursive WITH query's read of itself, inside the query: it has no subquery of its own. */
    kv_bool(s, "recursive", pItem->fg.isRecursive);
    if( pItem->pSTab ){
      tableSeen(pItem->pSTab);
      if( IsView(pItem->pSTab) ) kv_bool(s, "view", 1);
      if( IsVirtual(pItem->pSTab) ) kv_bool(s, "virtual", 1);
    }
    key(s, "subquery");
    if( pItem->fg.isSubquery && pItem->u4.pSubq ) selectJson(s, pItem->u4.pSubq->pSelect); else sqlite3_str_appendall(s, "null");
    sqlite3_str_appendchar(s, 1, ',');
    key(s, "on"); if( !pItem->fg.isUsing && pItem->u3.pOn ) expr(s, pItem->u3.pOn, p); else sqlite3_str_appendall(s, "null");
    sqlite3_str_appendchar(s, 1, ',');
    closeObj(s); sqlite3_str_appendchar(s, 1, ',');
  }
  closeArr(s); sqlite3_str_appendchar(s, 1, ',');
  key(s, "where"); expr(s, p->pWhere, p); sqlite3_str_appendchar(s, 1, ',');
  key(s, "groupBy"); exprList(s, p->pGroupBy, p); sqlite3_str_appendchar(s, 1, ',');
  key(s, "having"); expr(s, p->pHaving, p); sqlite3_str_appendchar(s, 1, ',');
  key(s, "orderBy"); exprList(s, pTop->pOrderBy, pTop); sqlite3_str_appendchar(s, 1, ',');
  key(s, "limit"); expr(s, pTop->pLimit ? pTop->pLimit->pLeft : 0, pTop); sqlite3_str_appendchar(s, 1, ',');
  key(s, "offset"); expr(s, pTop->pLimit ? pTop->pLimit->pRight : 0, pTop); sqlite3_str_appendchar(s, 1, ',');
  kv_bool(s, "distinct", (p->selFlags & SF_Distinct)!=0);
  kv_bool(s, "hasAgg", (p->selFlags & SF_HasAgg)!=0);
  kv_bool(s, "aggregate", (p->selFlags & SF_Aggregate)!=0);
  kv_bool(s, "values", (p->selFlags & SF_Values)!=0);
  kv_bool(s, "window", p->pWin!=0);
  if( p->pPrior ){
    kv_str(s, "op", p->op==TK_ALL ? "union all" : p->op==TK_UNION ? "union" : p->op==TK_INTERSECT ? "intersect" : "except");
    key(s, "prior"); selectJson(s, p->pPrior); sqlite3_str_appendchar(s, 1, ',');
  }
  closeObj(s);
}

/* ---- the hooks ------------------------------------------------------------------------------- */

static int skipHook(Parse *pParse){ return pParse->nErr || pParse->db->init.busy; }

static void selectHook(Parse *pParse, Select *p, SelectDest *pDest){
  if( skipHook(pParse) ) return;
  if( p==gInsertSelect ){
    gInsertSelect = 0;
    selectJson(gStmt, p);
  }else if( !gCaptured && pDest->eDest==SRT_Output ){
    gCaptured = 1;
    selectJson(gStmt, p);
  }
}
/* SQLite codes the SELECT that feeds an INSERT before it reaches the INSERT hook: this names it,
** and the SELECT hook serializes it as resolved, before the code generator rewrites it. */
static void insertSelectHook(Parse *pParse, Select *pSelect){
  if( skipHook(pParse) || gCaptured || sqlite3_str_length(gStmt) ) return;
  gInsertSelect = pSelect;
}

static int insertColumnIndex(Table *pTab, IdList *pColumn, int i){
  int j, seen = 0;
  if( pColumn ){
    if( i>=pColumn->nId ) return -1;
    for(j=0; j<pTab->nCol; j++) if( sqlite3StrICmp(pTab->aCol[j].zCnName, pColumn->a[i].zName)==0 ) return j;
    /* The rowid by one of its names is the INTEGER PRIMARY KEY column, where the table has one. */
    return sqlite3IsRowid(pColumn->a[i].zName) ? pTab->iPKey : -1;
  }
  for(j=0; j<pTab->nCol; j++){
    if( pTab->aCol[j].colFlags & COLFLAG_NOINSERT ) continue;
    if( seen++==i ) return j;
  }
  return -1;
}
static const char *onErrorName(int onError){
  switch( onError ){
    case OE_Ignore: return "ignore"; case OE_Replace: return "replace"; case OE_Abort: return "abort";
    case OE_Rollback: return "rollback"; case OE_Fail: return "fail"; default: return "default";
  }
}
static void upsert(sqlite3_str *s, Upsert *pUpsert){
  int i;
  if( pUpsert==0 ){ sqlite3_str_appendall(s, "null"); return; }
  sqlite3_str_appendchar(s, 1, '{');
  kv_bool(s, "doUpdate", pUpsert->isDoUpdate);
  key(s, "set"); sqlite3_str_appendchar(s, 1, '[');
  for(i=0; pUpsert->pUpsertSet && i<pUpsert->pUpsertSet->nExpr; i++){
    sqlite3_str_appendchar(s, 1, '{');
    kv_str(s, "column", pUpsert->pUpsertSet->a[i].zEName);
    key(s, "expr"); expr(s, pUpsert->pUpsertSet->a[i].pExpr, 0); sqlite3_str_appendchar(s, 1, ',');
    closeObj(s); sqlite3_str_appendchar(s, 1, ',');
  }
  closeArr(s); sqlite3_str_appendchar(s, 1, ',');
  key(s, "where"); expr(s, pUpsert->pUpsertWhere, 0); sqlite3_str_appendchar(s, 1, ',');
  closeObj(s);
}
static void insertHook(Parse *pParse, Table *pTab, IdList *pColumn, ExprList *pList, Select *pSelect, int onError, Upsert *pUpsert){
  int i, n;
  if( skipHook(pParse) || gCaptured ) return;
  gCaptured = 1;
  tableSeen(pTab);
  sqlite3_str_appendchar(gDml, 1, '{');
  kv_str(gDml, "kind", "insert");
  kv_str(gDml, "table", pTab->zName);
  kv_str(gDml, "onError", onErrorName(onError));
  key(gDml, "upsert"); upsert(gDml, pUpsert); sqlite3_str_appendchar(gDml, 1, ',');
  n = pList ? pList->nExpr : pSelect && pSelect->pEList ? pSelect->pEList->nExpr : 0;
  key(gDml, "columns"); sqlite3_str_appendchar(gDml, 1, '[');
  for(i=0; i<n; i++){
    int j = insertColumnIndex(pTab, pColumn, i);
    js(gDml, j>=0 ? pTab->aCol[j].zCnName : 0); sqlite3_str_appendchar(gDml, 1, ',');
  }
  closeArr(gDml); sqlite3_str_appendchar(gDml, 1, ',');
  key(gDml, "values"); if( pList ) exprList(gDml, pList, 0); else sqlite3_str_appendall(gDml, "null");
  sqlite3_str_appendchar(gDml, 1, ',');
  if( pSelect ){
    int rows = 0;
    Select *q;
    if( pSelect->selFlags & SF_Values ){ for(q=pSelect; q; q=q->pPrior) rows++; }
    kv_int(gDml, "rows", (pSelect->selFlags & SF_Values) ? rows : -1);
  }else{
    kv_int(gDml, "rows", 1);
  }
  closeObj(gDml);
}
static void updateHook(Parse *pParse, Table *pTab, ExprList *pChanges, Expr *pWhere, int *aXRef, SrcList *pTabList, int onError, Upsert *pUpsert){
  int i, j;
  if( skipHook(pParse) || gCaptured ) return;
  gCaptured = 1;
  tableSeen(pTab);
  sqlite3_str_appendchar(gDml, 1, '{');
  kv_str(gDml, "kind", pUpsert ? "upsert" : "update");
  kv_str(gDml, "table", pTab->zName);
  kv_str(gDml, "onError", onErrorName(onError));
  key(gDml, "set"); sqlite3_str_appendchar(gDml, 1, '[');
  for(i=0; pChanges && i<pChanges->nExpr; i++){
    int col = -1;
    for(j=0; j<pTab->nCol; j++) if( aXRef[j]==i ) col = j;
    sqlite3_str_appendchar(gDml, 1, '{');
    kv_str(gDml, "column", col>=0 ? pTab->aCol[col].zCnName : 0);
    key(gDml, "expr"); expr(gDml, pChanges->a[i].pExpr, 0); sqlite3_str_appendchar(gDml, 1, ',');
    closeObj(gDml); sqlite3_str_appendchar(gDml, 1, ',');
  }
  closeArr(gDml); sqlite3_str_appendchar(gDml, 1, ',');
  key(gDml, "where"); expr(gDml, pWhere, 0); sqlite3_str_appendchar(gDml, 1, ',');
  kv_int(gDml, "fromItems", pTabList ? pTabList->nSrc : 1);
  closeObj(gDml);
}
static void deleteHook(Parse *pParse, Table *pTab, Expr *pWhere){
  if( skipHook(pParse) || gCaptured ) return;
  gCaptured = 1;
  tableSeen(pTab);
  sqlite3_str_appendchar(gDml, 1, '{');
  kv_str(gDml, "kind", "delete");
  kv_str(gDml, "table", pTab->zName);
  key(gDml, "where"); expr(gDml, pWhere, 0); sqlite3_str_appendchar(gDml, 1, ',');
  closeObj(gDml);
}
static void returningHook(Parse *pParse, Table *pTab, ExprList *pNew){
  int i;
  if( skipHook(pParse) || sqlite3_str_length(gReturning) ) return;
  tableSeen(pTab);
  sqlite3_str_appendchar(gReturning, 1, '[');
  for(i=0; pNew && i<pNew->nExpr; i++){
    sqlite3_str_appendchar(gReturning, 1, '{');
    kv_str(gReturning, "name", pNew->a[i].zEName);
    key(gReturning, "expr"); expr(gReturning, pNew->a[i].pExpr, 0); sqlite3_str_appendchar(gReturning, 1, ',');
    closeObj(gReturning); sqlite3_str_appendchar(gReturning, 1, ',');
  }
  closeArr(gReturning);
}

/* ---- tables: columns and unique keys ------------------------------------------------------- */

/* Whether a CHECK refuses NULL in column iCol: one of its conjuncts tests the column IS NOT NULL,
** which NULL makes false where any other test of it would be NULL, and so pass. The CHECK's names
** are resolved (TK_COLUMN) once the table is built, and still names (TK_ID) before. */
static int refusesNull(Expr *p, Table *pTab, int iCol){
  Expr *x;
  if( p==0 ) return 0;
  if( p->op==TK_AND ) return refusesNull(p->pLeft, pTab, iCol) || refusesNull(p->pRight, pTab, iCol);
  if( p->op!=TK_NOTNULL ) return 0;
  for(x=p->pLeft; x && x->op==TK_COLLATE; x=x->pLeft){}
  if( x==0 ) return 0;
  if( x->op==TK_COLUMN ) return x->iColumn==iCol;
  if( x->op==TK_ID ) return sqlite3StrICmp(x->u.zToken, pTab->aCol[iCol].zCnName)==0;
  return 0;
}
static int checkedNotNull(Table *pTab, int iCol){
#ifndef SQLITE_OMIT_CHECK
  int k;
  for(k=0; pTab->pCheck && k<pTab->pCheck->nExpr; k++){
    if( refusesNull(pTab->pCheck->a[k].pExpr, pTab, iCol) ) return 1;
  }
#endif
  return 0;
}

static void tables(sqlite3_str *s){
  int t, i, k;
  sqlite3_str_appendchar(s, 1, '{');
  for(t=0; t<gnTables; t++){
    Table *pTab = gTables[t];
    Index *pIdx;
    key(s, pTab->zName); sqlite3_str_appendchar(s, 1, '{');
    kv_bool(s, "strict", (pTab->tabFlags & TF_Strict)!=0);
    kv_bool(s, "withoutRowid", (pTab->tabFlags & TF_WithoutRowid)!=0);
    kv_str(s, "rowidAlias", pTab->iPKey>=0 ? pTab->aCol[pTab->iPKey].zCnName : 0);
    key(s, "columns"); sqlite3_str_appendchar(s, 1, '[');
    for(i=0; i<pTab->nCol; i++){
      Column *pCol = &pTab->aCol[i];
      sqlite3_str_appendchar(s, 1, '{');
      kv_str(s, "name", pCol->zCnName);
      kv_str(s, "declared", sqlite3ColumnType(pCol, ""));
      kv_str(s, "affinity", affName(pCol->affinity));
      kv_bool(s, "notNull", pCol->notNull!=0);
      kv_bool(s, "checkedNotNull", checkedNotNull(pTab, i));
      kv_bool(s, "pk", (pCol->colFlags & COLFLAG_PRIMKEY)!=0);
      kv_bool(s, "generated", (pCol->colFlags & COLFLAG_GENERATED)!=0);
      kv_bool(s, "hidden", (pCol->colFlags & COLFLAG_HIDDEN)!=0);
      closeObj(s); sqlite3_str_appendchar(s, 1, ',');
    }
    closeArr(s); sqlite3_str_appendchar(s, 1, ',');
    /* A unique key: every unique, non-partial index over plain columns, the INTEGER PRIMARY KEY, and the rowid itself. */
    key(s, "uniqueKeys"); sqlite3_str_appendchar(s, 1, '[');
    if( pTab->iPKey>=0 ){ sqlite3_str_appendchar(s, 1, '['); js(s, pTab->aCol[pTab->iPKey].zCnName); sqlite3_str_appendall(s, "],"); }
    else if( !(pTab->tabFlags & TF_WithoutRowid) && rowidName(pTab) ){ sqlite3_str_appendchar(s, 1, '['); js(s, rowidName(pTab)); sqlite3_str_appendall(s, "],"); }
    for(pIdx=pTab->pIndex; pIdx; pIdx=pIdx->pNext){
      int ok = pIdx->onError!=OE_None && pIdx->pPartIdxWhere==0 && !pIdx->bHasExpr;
      for(k=0; ok && k<pIdx->nKeyCol; k++) if( pIdx->aiColumn[k]<0 ) ok = 0;
      /* A comparison of the column uses the column's collation: an index under another one bounds
      ** it only when the column's is BINARY, which tells every value apart (NOCASE does not). */
      for(k=0; ok && k<pIdx->nKeyCol; k++){
        const char *zColl = sqlite3ColumnColl(&pTab->aCol[pIdx->aiColumn[k]]);
        const char *zIdxColl = pIdx->azColl[k];
        if( zColl==0 ) zColl = sqlite3StrBINARY;
        if( zIdxColl==0 ) zIdxColl = sqlite3StrBINARY;
        if( sqlite3StrICmp(zColl, zIdxColl)!=0 && sqlite3StrICmp(zColl, sqlite3StrBINARY)!=0 ) ok = 0;
      }
      if( !ok ) continue;
      sqlite3_str_appendchar(s, 1, '[');
      for(k=0; k<pIdx->nKeyCol; k++){ js(s, pTab->aCol[pIdx->aiColumn[k]].zCnName); sqlite3_str_appendchar(s, 1, ','); }
      closeArr(s); sqlite3_str_appendchar(s, 1, ',');
    }
    closeArr(s); sqlite3_str_appendchar(s, 1, ',');
    closeObj(s); sqlite3_str_appendchar(s, 1, ',');
  }
  closeObj(s);
}

/* ---- the schema replay and the entry point ------------------------------------------------- */

static void installHooks(int on){
  reylo_select_hook = on ? selectHook : 0;
  reylo_insert_hook = on ? insertHook : 0;
  reylo_update_hook = on ? updateHook : 0;
  reylo_delete_hook = on ? deleteHook : 0;
  reylo_returning_hook = on ? returningHook : 0;
  reylo_insert_select_hook = on ? insertSelectHook : 0;
}

/* A host function's stand-in: the replay only prepares, so it is never called. */
static void hostFunctionStub(sqlite3_context *ctx, int argc, sqlite3_value **argv){
  (void)argc; (void)argv;
  sqlite3_result_null(ctx);
}
static void hostFinalStub(sqlite3_context *ctx){ sqlite3_result_null(ctx); }

static void functionError(sqlite3_str *errors, const char *zName){
  sqlite3_str_appendchar(errors, 1, '{');
  kv_str(errors, "function", zName);
  kv_str(errors, "message", sqlite3_errmsg(gDb));
  closeObj(errors); sqlite3_str_appendchar(errors, 1, ',');
}

/* Whether a replay step that failed is no failure: the statement creates one of SQLite's own
** tables, which sqlite_schema lists like any other. The replay has made sqlite_sequence itself
** with the first AUTOINCREMENT table, and a virtual table's shadow tables with the virtual table;
** any other sqlite_ table (sqlite_stat1, or sqlite_sequence where no AUTOINCREMENT table is
** left) is made here as the plain table it is, which only a writable schema allows. */
static int internalTable(const char *zDdl, const char *zErr){
  static const char zReserved[] = "object name reserved for internal use: ";
  static const char zExists[] = " already exists";
  char *zErr2 = 0;
  int n, ok = 0;
  if( zErr==0 ) return 0;
  if( strncmp(zErr, zReserved, sizeof(zReserved)-1)==0 ){
    sqlite3_db_config(gDb, SQLITE_DBCONFIG_WRITABLE_SCHEMA, 1, (int*)0);
    ok = sqlite3_exec(gDb, zDdl, 0, 0, &zErr2)==SQLITE_OK;
    sqlite3_db_config(gDb, SQLITE_DBCONFIG_WRITABLE_SCHEMA, 0, (int*)0);
    if( !ok && zErr2 ){
      n = (int)strlen(zErr2);
      ok = n>(int)sizeof(zExists) && strcmp(&zErr2[n-(sizeof(zExists)-1)], zExists)==0;
    }
    sqlite3_free(zErr2);
    return ok;
  }
  n = (int)strlen(zErr);
  if( strncmp(zErr, "table ", 6)==0 && n>6+(int)sizeof(zExists)-1 && strcmp(&zErr[n-(sizeof(zExists)-1)], zExists)==0 ){
    char *zName = sqlite3_mprintf("%.*s", n-6-(int)(sizeof(zExists)-1), &zErr[6]);
    if( zName ){
      Table *pTab;
      sqlite3Dequote(zName);
      pTab = sqlite3FindTable(gDb, zName, "main");
      ok = pTab!=0 && (pTab->tabFlags & TF_Shadow)!=0;
      sqlite3_free(zName);
    }
  }
  return ok;
}

/* Replays the DDL into a fresh in-memory database; errors are reported, not fatal. The host's
** functions are registered first, so a view, an index or a CHECK that calls one replays too: the
** ones it registers on its connection (azFn), and of the ones its SQLite lists (azHost, with the
** argument count and 's', 'a' or 'w' for a scalar, an aggregate or a window function; nHost is
** negative when it lists none), every one this build has no function of that name for. */
static int openSchema(
  const char *const *azDdl, int nDdl, const char *const *azFn, const int *aFnArg, int nFn,
  const char *const *azHost, const int *aHostArg, const char *zHostType, int nHost, sqlite3_str *errors
){
  sqlite3_str *k = sqlite3_str_new(0);
  char *zKey;
  char *aMissing = 0;
  int i, rc;
  const int flags = SQLITE_UTF8|SQLITE_DETERMINISTIC|SQLITE_INNOCUOUS;
  for(i=0; i<nFn; i++){ sqlite3_str_appendf(k, "%s/%d", azFn[i], aFnArg[i]); sqlite3_str_appendchar(k, 1, '\2'); }
  for(i=0; i<nHost; i++){ sqlite3_str_appendf(k, "%s/%d/%c", azHost[i], aHostArg[i], zHostType[i]); sqlite3_str_appendchar(k, 1, '\3'); }
  for(i=0; i<nDdl; i++){ sqlite3_str_appendall(k, azDdl[i]); sqlite3_str_appendchar(k, 1, '\1'); }
  zKey = sqlite3_str_finish(k);
  if( zKey==0 ) zKey = sqlite3_mprintf("");
  if( gDb && gDdlKey && strcmp(gDdlKey, zKey)==0 ){ sqlite3_free(zKey); return SQLITE_OK; }
  if( gDb ){ sqlite3_close(gDb); gDb = 0; }
  sqlite3_free(gDdlKey); gDdlKey = zKey;
  rc = sqlite3_open(":memory:", &gDb);
  if( rc!=SQLITE_OK ) return rc;
  installHooks(0);
  for(i=0; i<nFn; i++){
    /* Deterministic and innocuous: whatever the host's own flags, the replay accepts every place
    ** the host's schema could already call it from. */
    if( sqlite3_create_function_v2(gDb, azFn[i], aFnArg[i], flags, 0, hostFunctionStub, 0, 0, 0)!=SQLITE_OK ) functionError(errors, azFn[i]);
  }
  /* Which of the host SQLite's functions this build lacks is settled before any stub is made: a
  ** stub for one argument count must not hide that the next row's count is missing too. */
  if( nHost>0 ) aMissing = sqlite3_malloc64((sqlite3_uint64)nHost);
  for(i=0; aMissing && i<nHost; i++) aMissing[i] = sqlite3FindFunction(gDb, azHost[i], -2, SQLITE_UTF8, 0)==0;
  for(i=0; aMissing && i<nHost; i++){
    int nArg = aHostArg[i]<0 ? -1 : aHostArg[i];
    if( !aMissing[i] ) continue;
    if( zHostType[i]=='w' ) rc = sqlite3_create_window_function(gDb, azHost[i], nArg, flags, 0, hostFunctionStub, hostFinalStub, hostFinalStub, hostFunctionStub, 0);
    else if( zHostType[i]=='a' ) rc = sqlite3_create_function_v2(gDb, azHost[i], nArg, flags, 0, 0, hostFunctionStub, hostFinalStub, 0);
    else rc = sqlite3_create_function_v2(gDb, azHost[i], nArg, flags, 0, hostFunctionStub, 0, 0, 0);
    if( rc!=SQLITE_OK ) functionError(errors, azHost[i]);
  }
  sqlite3_free(aMissing);
  for(i=0; i<nDdl; i++){
    char *zErr = 0;
    int nTry, rcExec = SQLITE_OK;
    /* A CHECK, an index's expression or a generated column that calls a function the host's
    ** connection does not have (an extension the application loads, such as sqlite-jsonschema's
    ** jsonschema_matches) stops CREATE TABLE and CREATE INDEX, though reading the table never
    ** calls it: SQLite resolves those expressions when it creates them. The function is the
    ** application's, so the replay stubs it, as it stubs the host's own, and runs the statement
    ** again. A statement that calls it itself is still refused by functionAuth. */
    static const char zNoFunction[] = "no such function: ";
    for(nTry=0; nTry<16; nTry++){
      rcExec = sqlite3_exec(gDb, azDdl[i], 0, 0, &zErr);
      if( rcExec==SQLITE_OK || zErr==0 || strncmp(zErr, zNoFunction, sizeof(zNoFunction)-1)!=0 ) break;
      if( sqlite3FindFunction(gDb, &zErr[sizeof(zNoFunction)-1], -2, SQLITE_UTF8, 0)!=0
       || sqlite3_create_function_v2(gDb, &zErr[sizeof(zNoFunction)-1], -1, flags, 0, hostFunctionStub, 0, 0, 0)!=SQLITE_OK ) break;
      sqlite3_free(zErr); zErr = 0;
    }
    if( rcExec!=SQLITE_OK && !internalTable(azDdl[i], zErr) ){
      sqlite3_str_appendchar(errors, 1, '{');
      kv_int(errors, "index", i);
      kv_str(errors, "message", zErr);
      closeObj(errors); sqlite3_str_appendchar(errors, 1, ',');
    }
    sqlite3_free(zErr);
  }
  return SQLITE_OK;
}

/* ---- the functions of the host's SQLite ------------------------------------------------------ */

static const char *const *gHost;  /* the names the host's SQLite lists */
static int gnHost;
static const char *const *gExtra; /* the names the host registers on its connection */
static int gnExtra;
static int gnDenied;              /* calls refused while this statement was prepared */
static char *gDeniedView;         /* the view the first of them is in, if it is in one */

/* SQLite asks this of every function a statement calls, the ones in the views it reads included.
** A function this build has and the host's SQLite does not list is refused, and the prepare fails
** at the call. Both lists are pragma_function_list's, which shows no internal function, so one
** of those is never refused. */
static int functionAuth(void *pArg, int code, const char *z1, const char *zFn, const char *z3, const char *zInside){
  FuncDef *pDef;
  int i;
  (void)pArg; (void)z1; (void)z3;
  if( code!=SQLITE_FUNCTION || zFn==0 ) return SQLITE_OK;
  for(i=0; i<gnHost; i++) if( sqlite3StrICmp(gHost[i], zFn)==0 ) return SQLITE_OK;
  for(i=0; i<gnExtra; i++) if( sqlite3StrICmp(gExtra[i], zFn)==0 ) return SQLITE_OK;
  pDef = sqlite3FindFunction(gDb, zFn, -2, SQLITE_UTF8, 0);
  if( pDef==0 || (pDef->funcFlags & SQLITE_FUNC_INTERNAL)!=0 ) return SQLITE_OK;
  if( gnDenied++==0 && zInside ){
    Table *pTab = sqlite3FindTable(gDb, zInside, 0);
    if( pTab && IsView(pTab) ) gDeniedView = sqlite3_mprintf("%s", zInside);
  }
  return SQLITE_DENY;
}

/* The schema a failed lookup named, where the replay has no schema of that name: for "no such
** table: baz.users", the `baz` of a `baz.users` in the statement, with the reference's offset
** and length. SQLite's own tokenizer finds it, so quoting and comments are read as SQLite reads
** them. 0 when the message is another one or names a schema that exists (main, temp). */
static char *missingSchema(const char *zSql, const char *zMsg, int *piOfst, int *pnLen){
  static const char zPrefix[] = "no such table: ";
  const unsigned char *z = (const unsigned char*)zSql;
  const unsigned char *aTok[3] = {0, 0, 0};
  int aLen[3] = {0, 0, 0};
  int aType[3] = {0, 0, 0};
  if( strncmp(zMsg, zPrefix, sizeof(zPrefix)-1)!=0 ) return 0;
  while( *z ){
    int tk = 0;
    int n = (int)sqlite3GetToken(z, &tk);
    if( n<=0 ) break;
    if( tk!=TK_SPACE && tk!=TK_COMMENT ){
      int name = tk==TK_ID || tk==TK_STRING || (tk<TK_SPACE && sqlite3ParserFallback(tk)==TK_ID);
      aTok[0] = aTok[1]; aLen[0] = aLen[1]; aType[0] = aType[1];
      aTok[1] = aTok[2]; aLen[1] = aLen[2]; aType[1] = aType[2];
      aTok[2] = z; aLen[2] = n; aType[2] = name ? 1 : tk==TK_DOT ? 2 : 0;
      if( aType[0]==1 && aType[1]==2 && aType[2]==1 ){
        char *zDb = sqlite3_mprintf("%.*s", aLen[0], aTok[0]);
        char *zTab = sqlite3_mprintf("%.*s", aLen[2], aTok[2]);
        char *zFull = 0;
        if( zDb && zTab ){
          sqlite3Dequote(zDb); sqlite3Dequote(zTab);
          zFull = sqlite3_mprintf("%s%s.%s", zPrefix, zDb, zTab);
        }
        if( zFull && strcmp(zFull, zMsg)==0 && sqlite3FindDbName(gDb, zDb)<0 ){
          *piOfst = (int)(aTok[0] - (const unsigned char*)zSql);
          *pnLen = (int)(aTok[2] + aLen[2] - aTok[0]);
          sqlite3_free(zTab); sqlite3_free(zFull);
          return zDb;
        }
        sqlite3_free(zDb); sqlite3_free(zTab); sqlite3_free(zFull);
      }
    }
    z += n;
  }
  return 0;
}

/* How deep the user's statement may nest: see reylo_sqlite_analyze. */
#define REYLO_SQLITE_EXPR_DEPTH 350

/*
** Analyzes one statement against the schema DDL, with the host's scalar functions (azFn[i] taking
** aFnArg[i] arguments, -1 for any number) registered on the replay, and the functions of the
** host's SQLite (azHost[i] taking aHostArg[i] arguments, of type zHostType[i]; nHost is negative
** when the host did not list them, and every function this build has is then taken to exist).
** Returns a JSON document (free with reylo_sqlite_free): { statement, dml, returning, tables,
** columns, params, readonly, error, schemaErrors }.
*/
char *reylo_sqlite_analyze(
  const char *const *azDdl, int nDdl, const char *zSql, const char *const *azFn, const int *aFnArg, int nFn,
  const char *const *azHost, const int *aHostArg, const char *zHostType, int nHost
){
  sqlite3_str *out = sqlite3_str_new(0);
  sqlite3_str *schemaErrors = sqlite3_str_new(0);
  sqlite3_stmt *pStmt = 0;
  const char *zTail = 0;
  int rc, i, n, nDepth;
  sqlite3_str_appendchar(schemaErrors, 1, '[');
  rc = openSchema(azDdl, nDdl, azFn, aFnArg, nFn, azHost, aHostArg, zHostType, nHost, schemaErrors);
  closeArr(schemaErrors);
  sqlite3_str_appendchar(out, 1, '{');
  if( rc!=SQLITE_OK ){
    key(out, "error"); sqlite3_str_appendall(out, "{\"message\":\"cannot open the schema replay\",\"offset\":-1}");
    closeObj(out);
    sqlite3_free(sqlite3_str_finish(schemaErrors));
    return sqlite3_str_finish(out);
  }
  gStmt = sqlite3_str_new(0); gDml = sqlite3_str_new(0); gReturning = sqlite3_str_new(0);
  gCaptured = 0; gInsertSelect = 0; gnTables = 0;
  gHost = azHost; gnHost = nHost; gExtra = azFn; gnExtra = nFn; gnDenied = 0; gDeniedView = 0;
  if( nHost>=0 ) sqlite3_set_authorizer(gDb, functionAuth, 0);
  installHooks(1);
  /* The statement's expressions nest at most REYLO_SQLITE_EXPR_DEPTH deep, 350 where SQLite's own
  ** bound is 1,000: the analysis of a tree that deep is what the WebAssembly engine can afford on
  ** the JavaScript engine's stack beside a host that holds 45% of it, as libpg_query's bound is
  ** for Postgres (core/src/stack.rs). At 500, a nested CASE or function call needed 55% of Node's
  ** stack before V8 optimized the engine and 62% after on Node 24; at 350 the deepest statement
  ** of every shape needs at most 42% (measured on Node 24 and 26, 2026-10-21). The schema's own
  ** statements keep SQLite's bound. */
  nDepth = sqlite3_limit(gDb, SQLITE_LIMIT_EXPR_DEPTH, REYLO_SQLITE_EXPR_DEPTH);
  rc = sqlite3_prepare_v2(gDb, zSql, -1, &pStmt, &zTail);
  sqlite3_limit(gDb, SQLITE_LIMIT_EXPR_DEPTH, nDepth);
  installHooks(0);
  if( nHost>=0 ) sqlite3_set_authorizer(gDb, 0, 0);
  gInsertSelect = 0;
  if( rc!=SQLITE_OK ){
    static const char zDenied[] = "not authorized to use function: ";
    const char *zMsg = sqlite3_errmsg(gDb);
    int iOfst = sqlite3_error_offset(gDb);
    int nLen = 0;
    char *zSchema = missingSchema(zSql, zMsg, &iOfst, &nLen);
    key(out, "error"); sqlite3_str_appendchar(out, 1, '{');
    if( gnDenied && strncmp(zMsg, zDenied, sizeof(zDenied)-1)==0 ){
      /* The call is in the view's text, not the statement's: its offset would point nowhere. */
      kv_str(out, "missingFunction", &zMsg[sizeof(zDenied)-1]);
      if( gDeniedView ){ kv_str(out, "view", gDeniedView); iOfst = -1; }
    }
    kv_str(out, "message", zMsg);
    kv_int(out, "offset", iOfst);
    if( zSchema ){ kv_str(out, "missingSchema", zSchema); kv_int(out, "length", nLen); }
    closeObj(out); sqlite3_str_appendchar(out, 1, ',');
    sqlite3_free(zSchema);
  }else{
    key(out, "statement"); if( sqlite3_str_length(gStmt) ) sqlite3_str_appendall(out, sqlite3_str_value(gStmt)); else sqlite3_str_appendall(out, "null");
    sqlite3_str_appendchar(out, 1, ',');
    key(out, "dml"); if( sqlite3_str_length(gDml) ) sqlite3_str_appendall(out, sqlite3_str_value(gDml)); else sqlite3_str_appendall(out, "null");
    sqlite3_str_appendchar(out, 1, ',');
    key(out, "returning"); if( sqlite3_str_length(gReturning) ) sqlite3_str_appendall(out, sqlite3_str_value(gReturning)); else sqlite3_str_appendall(out, "null");
    sqlite3_str_appendchar(out, 1, ',');
    n = sqlite3_column_count(pStmt);
    key(out, "columns"); sqlite3_str_appendchar(out, 1, '[');
    for(i=0; i<n; i++){
      sqlite3_str_appendchar(out, 1, '{');
      kv_str(out, "name", sqlite3_column_name(pStmt, i));
      kv_str(out, "declared", sqlite3_column_decltype(pStmt, i));
      kv_str(out, "table", sqlite3_column_table_name(pStmt, i));
      kv_str(out, "column", sqlite3_column_origin_name(pStmt, i));
      closeObj(out); sqlite3_str_appendchar(out, 1, ',');
    }
    closeArr(out); sqlite3_str_appendchar(out, 1, ',');
    n = sqlite3_bind_parameter_count(pStmt);
    key(out, "params"); sqlite3_str_appendchar(out, 1, '[');
    for(i=1; i<=n; i++){
      sqlite3_str_appendchar(out, 1, '{');
      kv_int(out, "index", i);
      kv_str(out, "name", sqlite3_bind_parameter_name(pStmt, i));
      closeObj(out); sqlite3_str_appendchar(out, 1, ',');
    }
    closeArr(out); sqlite3_str_appendchar(out, 1, ',');
    kv_bool(out, "readonly", sqlite3_stmt_readonly(pStmt));
    kv_bool(out, "tail", zTail && zTail[0] && sqlite3_complete(zTail)==0 ? 1 : (zTail && strspn(zTail, " \t\r\n;")!=strlen(zTail)));
    key(out, "tables"); tables(out); sqlite3_str_appendchar(out, 1, ',');
  }
  key(out, "schemaErrors"); sqlite3_str_appendall(out, sqlite3_str_value(schemaErrors) ? sqlite3_str_value(schemaErrors) : "[]");
  sqlite3_str_appendchar(out, 1, ',');
  closeObj(out);
  sqlite3_finalize(pStmt);
  sqlite3_free(sqlite3_str_finish(gStmt)); sqlite3_free(sqlite3_str_finish(gDml)); sqlite3_free(sqlite3_str_finish(gReturning));
  sqlite3_free(sqlite3_str_finish(schemaErrors));
  gStmt = gDml = gReturning = 0;
  sqlite3_free(gDeniedView); gDeniedView = 0;
  gHost = gExtra = 0; gnHost = gnExtra = 0;
  return sqlite3_str_finish(out);
}

void reylo_sqlite_free(char *p){ sqlite3_free(p); }

#ifdef REYLO_SQLITE_MAIN
/* reylo_sqlite_check <database file> <statements file>: the DDL of the file, one JSON line per statement. */
int main(int argc, char **argv){
  sqlite3 *db; sqlite3_stmt *st; FILE *f; char line[8192];
  const char *azDdl[1024]; int nDdl = 0;
  if( argc<3 ){ fprintf(stderr, "usage: reylo_sqlite_check <db> <statements>\n"); return 2; }
  if( sqlite3_open_v2(argv[1], &db, SQLITE_OPEN_READONLY, 0)!=SQLITE_OK ){ fprintf(stderr, "open: %s\n", sqlite3_errmsg(db)); return 1; }
  sqlite3_prepare_v2(db, "select sql from sqlite_schema where sql is not null order by rowid", -1, &st, 0);
  while( sqlite3_step(st)==SQLITE_ROW && nDdl<1024 ) azDdl[nDdl++] = strdup((const char*)sqlite3_column_text(st, 0));
  sqlite3_finalize(st); sqlite3_close(db);
  f = fopen(argv[2], "r");
  if( f==0 ){ fprintf(stderr, "cannot read %s\n", argv[2]); return 1; }
  while( fgets(line, sizeof line, f) ){
    char *nl = strchr(line, '\n'); if( nl ) *nl = 0;
    if( line[0]==0 || line[0]=='#' ) continue;
    char *json = reylo_sqlite_analyze(azDdl, nDdl, line, 0, 0, 0, 0, 0, 0, -1);
    printf("%s\n", json);
    reylo_sqlite_free(json);
  }
  return 0;
}
#endif
