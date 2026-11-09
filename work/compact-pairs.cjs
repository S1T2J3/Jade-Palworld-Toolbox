const {recipeCell,recipeEmpty,recipeSpecial,recipeFirstChild}=require('./ui/recipe-index');

module.exports=function compactPairs({meta,species,pairs}) {
  const size=species.length,special={};
  if(size+recipeFirstChild>=0xd800) throw Error('Too many species to encode in one cell');
  const cells=new Uint16Array(size*(size+1)/2).fill(recipeEmpty);
  let previous=-1;
  for(const row of pairs) {
    const [a,b,c]=row,index=recipeCell(a,b,size);
    if(![a,b,c].every(i=>Number.isInteger(i)&&i>=0&&i<size)||a>b||index<previous) {
      throw Error('Expected canonical, ordered breeding pairs');
    }
    previous=index;
    if(row.length===3 && cells[index]===recipeEmpty) {
      cells[index]=recipeFirstChild+c;
    } else {
      if(!special[index]) special[index]=[];
      if(cells[index]>=recipeFirstChild) special[index].push([a,b,cells[index]-recipeFirstChild]);
      special[index].push(row);
      cells[index]=recipeSpecial;
    }
  }
  // Encode only BMP code points, so a cell is indexable without a second binary buffer.
  const text=Array.from(cells,code=>String.fromCharCode(code)).join('');
  return {meta,species,recipeIndex:{count:pairs.length,cells:text,special}};
};
