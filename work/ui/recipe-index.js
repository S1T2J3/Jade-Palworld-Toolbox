// One cell per unordered pair, with the upper triangle stored row by row.
// The two reserved values distinguish impossible crosses from special recipes.
const recipeEmpty=0x100,recipeSpecial=0x101,recipeFirstChild=0x102;
function recipeCell(a,b,size) {
  return a*size-a*(a+1)/2+b;
}

function createRecipeIndex(db) {
  const size=db.species.length;
  if(Array.isArray(db.pairs)) {
    const pairs=db.pairs;
    return {
      count:pairs.length,all:()=>pairs,
      parent:id=>pairs.filter(row=>row[0]===id||row[1]===id),
      child:id=>pairs.filter(row=>row[2]===id)
    };
  }
  const {cells,special,count}=db.recipeIndex;
  let complete;
  function addCell(rows,a,b,index,child) {
    const code=cells.charCodeAt(index);
    if(code===recipeSpecial) {
      for(const row of special[index]) if(child===undefined||row[2]===child) rows.push(row);
    } else if(code>=recipeFirstChild && (child===undefined||code===recipeFirstChild+child)) {
      rows.push([a,b,code-recipeFirstChild]);
    }
  }
  function walk(child) {
    const rows=[];
    let index=0;
    for(let a=0;a<size;a++) for(let b=a;b<size;b++) addCell(rows,a,b,index++,child);
    return rows;
  }
  return {
    count,
    parent(id) {
      const rows=[];
      if(Number.isInteger(id)&&id>=0&&id<size) for(let other=0;other<size;other++) {
        const a=Math.min(id,other),b=Math.max(id,other);
        addCell(rows,a,b,recipeCell(a,b,size));
      }
      return rows;
    },
    child:id=>Number.isInteger(id)&&id>=0&&id<size?walk(id):[],
    all(){return complete??=walk();}
  };
}
if(typeof module!=='undefined' && module.exports) module.exports={createRecipeIndex,recipeCell,recipeEmpty,recipeSpecial,recipeFirstChild};
