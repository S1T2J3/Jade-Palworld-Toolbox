use serde_json::{json,Value};
use std::collections::{HashMap,HashSet};

fn arr(v:&Value)->&[Value]{v.as_array().map(Vec::as_slice).unwrap_or(&[])}
fn strv(v:&Value)->&str{v.as_str().unwrap_or("")}
fn failure(status:&str,missing:Vec<String>)->Value{json!({"status":status,"missing":missing,"generations":null,"steps":[],"founders":[],"targetId":null})}
fn sex_name(s:usize)->&'static str{if s==0{"M"}else{"F"}}
fn successful(v:&Value)->bool{matches!(strv(&v["status"]),"ok"|"owned")}
fn route_score(v:&Value)->(i64,usize,usize,i64){let ops=arr(&v["surgery"]);(v["generations"].as_i64().unwrap_or(i64::MAX),ops.len(),ops.iter().filter(|o|o["mode"]=="consume").count(),ops.iter().map(|o|o["cost"].as_i64().unwrap_or(0)).sum())}

#[derive(Clone,Debug)]
struct Pair{a:usize,b:usize,c:usize,sex:Option<usize>}
fn parse_pairs(request:&Value,n:usize)->Result<Vec<Pair>,()> {
    let rows=request["pairs"].as_array().ok_or(())?;
    rows.iter().map(|r|{
        let a=r[0].as_u64().ok_or(())? as usize;let b=r[1].as_u64().ok_or(())? as usize;let c=r[2].as_u64().ok_or(())? as usize;
        if a>=n||b>=n||c>=n{return Err(())}
        let sex=match strv(&r[3]){"M"=>Some(0),"F"=>Some(1),_=>None};
        Ok(Pair{a,b,c,sex})
    }).collect()
}

/// Expand the compact upper-triangle table without changing source row order.
pub fn recipe_pairs(db:&Value)->Vec<Value>{
    if let Some(rows)=db["pairs"].as_array(){return rows.clone()}
    let n=arr(&db["species"]).len();let cells:Vec<u16>=strv(&db["recipeIndex"]["cells"]).encode_utf16().collect();
    let mut rows=Vec::new();let mut index=0;
    for a in 0..n{for b in a..n{
        let code=cells.get(index).copied().unwrap_or(0x100);
        if code==0x101 {rows.extend(arr(&db["recipeIndex"]["special"][index.to_string()]).iter().cloned());}
        else if code>=0x102 {rows.push(json!([a,b,code-0x102]));}
        index+=1;
    }}rows
}

fn combine(left:u32,right:u32,all:usize,all_sets:u32,memo:&mut HashMap<u64,u32>)->u32{
    if left==0||right==0{return 0} if left==1{return right} if right==1||left==all_sets{return left} if right==all_sets{return right}
    let key=(left.min(right) as u64)*65536+left.max(right) as u64;
    if let Some(v)=memo.get(&key){return *v}
    let mut result=0;
    for a in 0..=all {if left&(1<<a)==0{continue} for b in 0..=all {if right&(1<<b)!=0{result|=1<<(a|b)}}}
    memo.insert(key,result);result
}
fn random_sets(sets:u32,random:usize,all:usize,all_sets:u32,memo:&mut HashMap<u32,u32>)->u32{
    if random==0||sets==0||sets==all_sets{return sets} if let Some(v)=memo.get(&sets){return *v}
    let mut result=sets;
    for mask in 0..=all{if sets&(1<<mask)==0{continue}let mut bit=1;while bit<=random {if random&bit!=0{result|=1<<(mask|bit)}bit*=2}}
    memo.insert(sets,result);result
}
fn mask_for(keys:&Value,wanted:&[String])->usize{arr(keys).iter().fold(0,|mask,k|mask|wanted.iter().position(|w|k.as_str()==Some(w.as_str())).map(|i|1<<i).unwrap_or(0))}
fn trait_keys(mask:usize,wanted:&[String])->Vec<String>{wanted.iter().enumerate().filter(|(i,_)|mask&(1<<i)!=0).map(|(_,s)|s.clone()).collect()}

struct Witness{a:usize,b:usize,sex:usize,left:usize,right:usize,omitted:usize}
struct Reconstruction<'a>{
    count:usize,wanted:&'a [String],level:&'a [i32],founder_of:&'a [i32],records:&'a [Value],recipes:&'a [Vec<Pair>],random:usize,
    founders:Vec<(usize,usize,Value)>,bred:Vec<(usize,Value)>,
}
impl Reconstruction<'_>{
    fn slot(&self,species:usize,sex:usize,mask:usize)->usize{(species*2+sex)*self.count+mask}
    fn parents_for(&self,child:usize,mask:usize,generation:i32)->Option<Witness>{
        let mut best=None;let mut penalty=i64::MAX;
        for row in &self.recipes[child]{for sex in 0..2{
            if row.sex.is_some_and(|s|s!=sex){continue}
            if self.level[self.slot(row.a,sex,0)]<0||self.level[self.slot(row.b,1-sex,0)]<0{continue}
            let mut omitted=0;
            while omitted<=mask{
                if omitted==0||mask&self.random&omitted!=0{
                    let inherited=mask^omitted;let mut left=inherited;
                    loop{
                        let right=inherited^left;let da=self.level[self.slot(row.a,sex,left)];let db=self.level[self.slot(row.b,1-sex,right)];
                        if da>=0&&db>=0&&da.max(db)+1==generation{
                            let p=if omitted!=0{self.level.len() as i64}else{0}+da as i64+db as i64;
                            if p<penalty{best=Some(Witness{a:row.a,b:row.b,sex,left,right,omitted});penalty=p;if p==generation as i64-1{return best}}
                        }
                        if left==0{break}left=(left-1)&inherited;
                    }
                }
                omitted=if omitted==0{1}else{omitted*2};
            }
        }}best
    }
    fn add_sex(node:&mut Value,requested:Option<usize>){if let Some(s)=requested{let sex=Value::String(sex_name(s).into());let list=node["sexes"].as_array_mut().unwrap();if !list.contains(&sex){list.push(sex)}}}
    fn visit(&mut self,species:usize,mask:usize,sex:usize,requested:Option<usize>)->Result<String,()> {
        let position=self.slot(species,sex,mask);let depth=self.level[position];
        if depth==0{
            let index=self.founder_of[position] as usize;
            let existing=self.founders.iter().position(|(i,_,_)|*i==index);
            let idx=existing.unwrap_or_else(||{
                let actual=&self.records[index];
                self.founders.push((index,0,json!({"id":format!("f{index}"),"stockId":actual["id"],"species":species,"traits":actual["traits"],"sex":if strv(&actual["sex"]).is_empty(){"any"}else{strv(&actual["sex"])},"sexes":[]})));
                self.founders.len()-1
            });
            let (_,required,node)=&mut self.founders[idx];*required|=mask;Self::add_sex(node,requested);return Ok(strv(&node["id"]).into());
        }
        let key=species*self.count+mask;
        if !self.bred.iter().any(|(k,_)|*k==key){
            let w=self.parents_for(species,mask,depth).ok_or(())?;
            let mut parents=Vec::new();
            for (p,m,s) in [(w.a,w.left,w.sex),(w.b,w.right,1-w.sex)]{
                let id=self.visit(p,m,s,Some(s))?;
                parents.push(json!({"id":id,"species":p,"traits":trait_keys(m,self.wanted),"sex":sex_name(s)}));
            }
            self.bred.push((key,json!({"id":format!("p{key}"),"species":species,"traits":trait_keys(mask,self.wanted),"depth":depth,"parents":parents,"randomTraits":trait_keys(w.omitted,self.wanted),"sexes":[]})));
        }
        let node=&mut self.bred.iter_mut().find(|(k,_)|*k==key).unwrap().1;
        Self::add_sex(node,requested);Ok(strv(&node["id"]).into())
    }
}

fn solve_fixed(request:&Value,stock_override:Option<&[Value]>,random_override:Option<bool>,terminal:bool)->Value{
    let Some(base_n)=request["speciesCount"].as_u64().map(|n|n as usize) else{return failure("invalid",vec![])};
    let Some(base_target)=request["target"].as_u64().map(|n|n as usize) else{return failure("invalid",vec![])};
    if base_n==0||base_target>=base_n||base_n>100_000||!request["stock"].is_array()||!request["pairs"].is_array(){return failure("invalid",vec![])}
    let (n,target)=if terminal{(base_n+1,base_n)}else{(base_n,base_target)};
    let trait_value=request.get("traits").cloned().unwrap_or(json!([]));
    if !trait_value.is_array(){return failure("invalid",vec![])}
    let mut wanted=Vec::new();for t in arr(&trait_value){if strv(t).is_empty(){return failure("invalid",vec![])}let t=strv(t).to_string();if !wanted.contains(&t){wanted.push(t)}}
    if wanted.len()>4{return failure("invalid",vec![])}
    let records:Vec<Value>=stock_override.unwrap_or_else(||arr(&request["stock"])).iter().filter(|r|r["species"].as_u64().is_some_and(|s|s<(n as u64))&&r["traits"].is_array()).cloned().collect();
    if records.is_empty(){return failure("unreachable-species",vec![])}
    let count=1usize<<wanted.len();let all=count-1;let all_sets=(1u32<<count)-1;
    let operations:Vec<Value>=arr(&request["surgeryOptions"]).iter().filter(|o|wanted.iter().any(|w|o["key"].as_str()==Some(w.as_str()))).cloned().collect();
    let op_keys=json!(operations.iter().map(|o|o["key"].clone()).collect::<Vec<_>>());
    let inherited=all&!mask_for(&op_keys,&wanted);
    let subsets:Vec<u32>=(0..count).map(|m|(0..count).filter(|i|m&i==*i).fold(0,|v,i|v|(1<<i))).collect();
    let mut available=vec![0u32;n*2];let mut level=vec![-1i32;n*2*count];let mut founder_of=vec![-1i32;level.len()];let mut donor=0;
    for (index,record) in records.iter().enumerate(){let mask=mask_for(&record["traits"],&wanted);donor|=mask;
        let species=record["species"].as_u64().unwrap() as usize;
        for sex in 0..2{if matches!(strv(&record["sex"]),"M"|"F")&&strv(&record["sex"])!=sex_name(sex){continue}
            let s=species*2+sex;available[s]|=subsets[mask];
            for m in 0..=all{if mask&m!=m{continue}let p=s*count+m;if founder_of[p]>=0{continue}level[p]=0;founder_of[p]=index as i32;}
        }
    }
    let random=if random_override.unwrap_or_else(||request["allowRandom"].as_bool().unwrap_or(false)){mask_for(&request["randomEligible"],&wanted)&inherited}else{0};
    let missing=trait_keys((all&!donor)&inherited&!random,&wanted);if !missing.is_empty(){return failure("missing-traits",missing)}
    // An existing matching individual needs no recipe expansion. In the UI's
    // prefer-hatch flow this avoids parsing all 44,552 rows twice.
    let goal=target*2;
    let mut pairs=if (available[goal]|available[goal+1])&(1<<inherited)!=0{Vec::new()}else{let Ok(p)=parse_pairs(request,base_n)else{return failure("invalid",vec![])};p};
    if terminal{let extra:Vec<_>=pairs.iter().filter(|p|p.c==base_target).map(|p|Pair{a:p.a,b:p.b,c:base_n,sex:p.sex}).collect();pairs.extend(extra);}
    let mut recipes=vec![Vec::new();n];for p in &pairs{recipes[p.c].push(p.clone())}
    let mut union_memo=HashMap::new();let mut random_memo=HashMap::new();let mut generation=1;
    while (available[goal]|available[goal+1])&(1<<inherited)==0{
        let mut next=available.clone();let mut added=false;
        for p in &pairs{
            let c=p.c*2;if next[c]&next[c+1]==all_sets{continue}
            let a=p.a*2;let b=p.b*2;let mut outcomes=0;
            if p.sex!=Some(1){outcomes=combine(available[a],available[b+1],all,all_sets,&mut union_memo)}
            if p.sex!=Some(0){outcomes|=combine(available[a+1],available[b],all,all_sets,&mut union_memo)}
            outcomes=random_sets(outcomes,random,all,all_sets,&mut random_memo);if outcomes==0{continue}
            for s in c..=c+1{let mut new_sets=outcomes&!next[s];if new_sets==0{continue}added=true;next[s]|=new_sets;
                while new_sets!=0{let mask=new_sets.trailing_zeros() as usize;level[s*count+mask]=generation;new_sets&=new_sets-1;}
            }
        }
        if !added{return failure(if (available[goal]|available[goal+1])&1!=0{"unreachable-traits"}else{"unreachable-species"},vec![])}
        available=next;generation+=1;
    }
    let mut best:Option<(usize,usize,Value)>=None;
    for mask in inherited..=all{if mask&inherited!=inherited{continue}
        let surgery:Vec<_>=operations.iter().filter(|op|mask&(1<<wanted.iter().position(|w|op["key"].as_str()==Some(w.as_str())).unwrap())==0).cloned().collect();
        for sex in 0..2{let generations=level[(goal+sex)*count+mask];if generations<0{continue}
            let candidate=json!({"generations":generations,"surgery":surgery});
            if best.as_ref().is_none_or(|(_,_,b)|route_score(&candidate)<route_score(b)){best=Some((mask,sex,candidate));}
        }
    }
    let Some((mask,sex,best))=best else{return failure("invalid",vec![])};
    let mut witness=Reconstruction{count,wanted:&wanted,level:&level,founder_of:&founder_of,records:&records,recipes:&recipes,random,founders:Vec::new(),bred:Vec::new()};
    let Ok(target_id)=witness.visit(target,mask,sex,None) else{return failure("invalid",vec![])};
    let mut steps:Vec<Value>=witness.bred.into_iter().map(|(_,v)|v).collect();steps.sort_by_key(|v|v["depth"].as_i64().unwrap());
    let founders:Vec<Value>=witness.founders.into_iter().map(|(_,mask,mut node)|{node["requiredTraits"]=json!(trait_keys(mask,&wanted));node}).collect();
    json!({"status":if steps.is_empty(){"owned"}else{"ok"},"missing":[],"generations":best["generations"],"surgery":best["surgery"],"conditional":steps.iter().any(|s|!arr(&s["randomTraits"]).is_empty()),"steps":steps,"founders":founders,"targetId":target_id})
}

struct AssignmentSearch<'a>{request:&'a Value,originals:HashMap<String,Value>,solved:usize,incomplete:bool,best:Option<Value>,random:Option<bool>,terminal:bool}
impl AssignmentSearch<'_>{
    fn search(&mut self,candidates:Vec<Value>)->Option<Value>{
        if self.solved>=48{self.incomplete=true;return None}self.solved+=1;
        let mut result=solve_fixed(self.request,Some(&candidates),self.random,self.terminal);
        if !successful(&result){return Some(result)}
        if self.best.as_ref().is_some_and(|b|route_score(b)<=route_score(&result)){return self.best.clone()}
        let conflict=arr(&result["founders"]).iter().find(|f|self.originals.get(strv(&f["stockId"])).is_some_and(|r|r["sex"]=="unknown")&&arr(&f["sexes"]).len()>1).cloned();
        if conflict.is_none(){
            for f in result["founders"].as_array_mut().unwrap(){if self.originals.get(strv(&f["stockId"])).is_some_and(|r|r["sex"]=="unknown"){
                f["sex"]=json!("unknown");f["assumedSex"]=f["sexes"][0].clone();
            }}
            result["conditionalSex"]=json!(arr(&result["founders"]).iter().any(|f|!f["assumedSex"].is_null()));
            self.best=Some(result.clone());return Some(result)
        }
        let id=strv(&conflict.unwrap()["stockId"]).to_string();
        let Some(index)=candidates.iter().position(|r|strv(&r["id"])==id) else{return Some(failure("invalid",vec![]))};
        let mut failure=None;
        for sex in ["M","F"]{let mut branch=candidates.clone();branch[index]["sex"]=json!(sex);let answer=self.search(branch);
            if self.best.as_ref().is_some_and(|b|route_score(b)<=route_score(&result)){return self.best.clone()}
            if let Some(a)=answer{if failure.is_none()||a["status"]=="unreachable-traits"{failure=Some(a)}}
        }
        self.best.clone().or(failure)
    }
}
fn solve_assigned(request:&Value,random:Option<bool>,terminal:bool)->Value{
    if !arr(&request["stock"]).iter().any(|r|r["sex"]=="unknown"){return solve_fixed(request,None,random,terminal)}
    let known:Vec<Value>=arr(&request["stock"]).iter().filter(|r|!r.is_null()&&r["sex"]!="unknown").cloned().collect();
    let originals:HashMap<String,Value>=arr(&request["stock"]).iter().filter(|r|!r.is_null()).map(|r|(strv(&r["id"]).into(),r.clone())).collect();
    let mut stock=known.clone();stock.extend(arr(&request["stock"]).iter().filter(|r|r["sex"]=="unknown").cloned());
    let wanted=arr(&request["traits"]);let mut groups:Vec<(String,HashSet<String>,Vec<usize>)>=Vec::new();
    for (i,row) in stock.iter().enumerate(){
        let mut relevant:Vec<Value>=arr(&row["traits"]).iter().filter(|t|wanted.contains(t)).cloned().collect();relevant.sort_by(|a,b|strv(a).cmp(strv(b)));
        let key=json!([row["species"],relevant]).to_string();
        let pos=groups.iter().position(|(k,_,_)|*k==key).unwrap_or_else(||{groups.push((key,HashSet::new(),Vec::new()));groups.len()-1});
        if row["sex"]=="unknown"{groups[pos].2.push(i)}else{groups[pos].1.insert(strv(&row["sex"]).into());}
    }
    for (_,known,unknown) in groups{if unknown.is_empty()||(unknown.len()==1&&known.is_empty()){continue}let first=if known.contains("M"){1}else{0};for (n,i) in unknown.iter().enumerate(){stock[*i]["sex"]=json!(sex_name(if n%2==0{first}else{1-first}));}}
    let mut search=AssignmentSearch{request,originals,solved:0,incomplete:false,best:None,random,terminal};
    let result=search.search(stock);
    if search.incomplete{
        let confirmed=solve_fixed(request,Some(&known),random,terminal);
        if successful(&confirmed)&&search.best.as_ref().is_none_or(|b|route_score(&confirmed)<=route_score(b)){search.best=Some(confirmed)}
        if let Some(mut best)=search.best{best["minimumUnconfirmed"]=json!(true);return best}
        return failure("unconfirmed-sex",vec![])
    }
    let result=result.unwrap_or_else(||failure("unconfirmed-sex",vec![]));
    if result["conditionalSex"]==true&&!known.is_empty(){let confirmed=solve_fixed(request,Some(&known),random,terminal);if successful(&confirmed)&&route_score(&confirmed)<=route_score(&result){return confirmed}}
    result
}

pub fn trait_solve(request:&Value)->Value{
    trait_solve_options(request,None,None)
}
fn trait_solve_options(request:&Value,random:Option<bool>,terminal:Option<bool>)->Value{
    let terminal=terminal.unwrap_or(request["requireHatch"]==true);
    let mut result=solve_assigned(request,random,terminal);
    if terminal{let n=request["speciesCount"].as_u64();for step in result["steps"].as_array_mut().unwrap(){if step["species"].as_u64()==n{step["species"]=request["target"].clone()}}}result
}

/// UI entry point: prefer a new hatch even if a matching target already exists.
pub fn plan_request(request:&Value)->Value{
    let inherited=trait_solve_options(request,Some(false),None);
    let owned=if request["preferHatch"]==true&&inherited["status"]=="owned"{Some(inherited.clone())}else{None};
    let terminal=if owned.is_some(){Some(true)}else{None};
    let deterministic=if owned.is_some(){trait_solve_options(request,Some(false),terminal)}else{inherited};
    if successful(&deterministic)||deterministic["status"]=="invalid"{return deterministic}
    let result=if request["allowRandom"]==true&&!arr(&request["randomEligible"]).is_empty(){trait_solve_options(request,None,terminal)}else{deterministic};
    if let Some(mut owned)=owned{if result["status"]!="ok"{owned["hatchUnavailable"]=json!(true);return owned}}result
}

/// Species-only planning, retained as the structural lower bound for offspring.
pub fn warehouse_solve(pairs:&[Value],species_count:usize,owned:&[usize],max_steps:Option<usize>)->Value{
    let n=species_count;let mut depth=vec![usize::MAX;n];let mut recipe=vec![Value::Null;n];let mut bred_depth=depth.clone();let mut bred_recipe=recipe.clone();
    for &i in owned{if i<n{depth[i]=0}}
    if depth.contains(&0){for step in 1..=max_steps.unwrap_or(n+1){let mut added=false;
        for row in pairs{let (Some(a),Some(b),Some(c))=(row[0].as_u64(),row[1].as_u64(),row[2].as_u64())else{continue};let (a,b,c)=(a as usize,b as usize,c as usize);if a>=n||b>=n||c>=n{continue}
            if bred_depth[c]<step||depth[a].max(depth[b])!=step-1{continue}
            if !bred_recipe[c].is_null(){let p=&bred_recipe[c];if depth[a].min(depth[b])>=depth[p[0].as_u64().unwrap() as usize].min(depth[p[1].as_u64().unwrap() as usize]){continue}}
            bred_depth[c]=step;bred_recipe[c]=row.clone();if depth[c]!=0{depth[c]=step;recipe[c]=row.clone();added=true}
        }if !added{break}
    }}
    let encode=|v:Vec<usize>|v.into_iter().map(|d|if d==usize::MAX{Value::Null}else{json!(d)}).collect::<Vec<_>>();
    json!({"depth":encode(depth),"recipe":recipe,"bredDepth":encode(bred_depth),"bredRecipe":bred_recipe})
}
pub fn warehouse_route(plan:&Value,target:usize,include_owned:bool)->Vec<Value>{
    let final_row=&plan[if include_owned{"bredRecipe"}else{"recipe"}][target];if final_row.is_null(){return vec![]}
    fn visit(plan:&Value,index:usize,seen:&mut HashSet<usize>,ordered:&mut Vec<Value>){let row=&plan["recipe"][index];if row.is_null()||!seen.insert(index){return}visit(plan,row[0].as_u64().unwrap() as usize,seen,ordered);visit(plan,row[1].as_u64().unwrap() as usize,seen,ordered);ordered.push(row.clone())}
    let mut seen=HashSet::new();let mut ordered=Vec::new();visit(plan,final_row[0].as_u64().unwrap() as usize,&mut seen,&mut ordered);visit(plan,final_row[1].as_u64().unwrap() as usize,&mut seen,&mut ordered);
    ordered.sort_by_key(|r|plan["depth"][r[2].as_u64().unwrap() as usize].as_u64().unwrap_or(u64::MAX));ordered.push(final_row.clone());ordered
}

#[derive(Clone,Default)]
struct ParentGroup{parents:[Option<Value>;2],unknown:Vec<Value>,single:Option<Value>}
fn parent_groups(stock:&[Value])->Vec<(usize,ParentGroup)>{
    let mut groups:Vec<(usize,ParentGroup)>=Vec::new();
    for r in stock{let Some(s)=r["species"].as_u64()else{continue};let s=s as usize;
        let i=groups.iter().position(|(id,_)|*id==s).unwrap_or_else(||{groups.push((s,ParentGroup::default()));groups.len()-1});let g=&mut groups[i].1;
        match strv(&r["sex"]){"M"=>{if g.parents[0].is_none(){g.parents[0]=Some(r.clone())}},"F"=>{if g.parents[1].is_none(){g.parents[1]=Some(r.clone())}},_=>g.unknown.push(r.clone())}
    }
    for (_,g) in &mut groups{if g.parents.iter().all(Option::is_none)&&g.unknown.len()==1{g.single=Some(g.unknown[0].clone())}
        else{for sex in 0..2{if g.parents[sex].is_none()&&!g.unknown.is_empty(){g.parents[sex]=Some(g.unknown.remove(0))}}}}
    groups
}
fn offspring_parent(index:usize,sex:usize,record:Option<&Value>)->Value{
    json!({"species":index,"sex":sex_name(sex),"stockId":record.map(|r|r["id"].clone()).unwrap_or(Value::Null),"unknown":record.is_some_and(|r|r["sex"]=="unknown")})
}
fn offspring_fixed(pairs:&[Pair],n:usize,parents:&[Vec<Option<Value>>;2],items:&mut [Option<Value>]){
    let mut depths=[vec![-1i32;n],vec![-1i32;n]];
    for sex in 0..2{for i in 0..n{if parents[sex][i].is_some(){depths[sex][i]=0}}}
    let mut hatch=vec![-1i32;n];let mut secondary=vec![0i32;n];let mut witness=vec![None;n];let mut generation=1;
    loop{let mut added=false;
        for p in pairs{if hatch[p.c]>=0&&hatch[p.c]<generation{continue}
            for first in 0..2{if p.sex.is_some_and(|s|s!=first){continue}let da=depths[first][p.a];let db=depths[1-first][p.b];
                if da<0||db<0||da.max(db)!=generation-1{continue}let score=da.min(db);if hatch[p.c]==generation&&secondary[p.c]<=score{continue}
                hatch[p.c]=generation;secondary[p.c]=score;witness[p.c]=Some((p.a,p.b,first));
                for table in &mut depths{if table[p.c]<0{table[p.c]=generation;added=true}}
            }
        }if !added{break}generation+=1;
    }
    fn visit(c:usize,parents:&[Vec<Option<Value>>;2],hatch:&[i32],witness:&[Option<(usize,usize,usize)>],seen:&mut HashSet<usize>,steps:&mut Vec<Value>){
        if !seen.insert(c){return}let Some((a,b,first))=witness[c]else{return};let mut pair=Vec::new();
        for (k,index) in [a,b].iter().enumerate(){let s=if k==0{first}else{1-first};let record=parents[s][*index].as_ref();if record.is_none(){visit(*index,parents,hatch,witness,seen,steps)}pair.push(offspring_parent(*index,s,record));}
        steps.push(json!({"child":c,"depth":hatch[c],"parents":pair}));
    }
    for target in 0..n{if hatch[target]<0||items[target].as_ref().is_some_and(|i|i["depth"].as_i64().unwrap()<=hatch[target] as i64){continue}
        let mut steps=Vec::new();visit(target,parents,&hatch,&witness,&mut HashSet::new(),&mut steps);steps.sort_by_key(|s|s["depth"].as_i64().unwrap());
        items[target]=Some(json!({"id":target,"depth":hatch[target],"minimumCertain":true,"conditional":steps.iter().any(|s|arr(&s["parents"]).iter().any(|p|p["unknown"]==true)),"steps":steps}));
    }
}
fn validate_offspring_route(route:&[Value],structural:&Value,groups:&[(usize,ParentGroup)])->Option<Vec<Value>>{
    let group=|i:usize|groups.iter().find(|(id,_)|*id==i).map(|(_,g)|g);
    let limited=|i:usize|group(i).is_some_and(|g|g.parents.iter().any(Option::is_none));
    let mut assigned=HashMap::<usize,usize>::new();let mut edges=Vec::<(usize,Vec<usize>)>::new();
    fn pin(map:&mut HashMap<usize,usize>,index:usize,sex:usize)->bool{if let Some(s)=map.get(&index){*s==sex}else{map.insert(index,sex);true}}
    fn connect(edges:&mut Vec<(usize,Vec<usize>)>,a:usize,b:usize){if let Some((_,e))=edges.iter_mut().find(|(id,_)|*id==a){if !e.contains(&b){e.push(b)}}else{edges.push((a,vec![b]))}}
    for row in route{let a=row[0].as_u64()? as usize;let b=row[1].as_u64()? as usize;
        for id in [a,b]{if limited(id){let g=group(id)?;if g.single.is_none()&&!pin(&mut assigned,id,if g.parents[0].is_some(){0}else{1}){return None}}}
        if let Some(first)=match strv(&row[3]){"M"=>Some(0),"F"=>Some(1),_=>None}{
            if limited(a)&&!pin(&mut assigned,a,first){return None}if limited(b)&&!pin(&mut assigned,b,1-first){return None}
        }else if limited(a)&&limited(b){if a==b{return None}connect(&mut edges,a,b);connect(&mut edges,b,a);}
    }
    fn propagate(seeds:Vec<usize>,assigned:&mut HashMap<usize,usize>,edges:&[(usize,Vec<usize>)])->bool{
        let mut queue=seeds;let mut pos=0;while pos<queue.len(){let id=queue[pos];let needed=1-assigned[&id];
            if let Some((_,neighbors))=edges.iter().find(|(i,_)|*i==id){for neighbor in neighbors{if let Some(sex)=assigned.get(neighbor){if *sex!=needed{return false}}else{assigned.insert(*neighbor,needed);queue.push(*neighbor)}}}pos+=1;
        }true
    }
    if !propagate(assigned.keys().copied().collect(),&mut assigned,&edges){return None}
    for (id,_) in &edges{if !assigned.contains_key(id){assigned.insert(*id,0);if !propagate(vec![*id],&mut assigned,&edges){return None}}}
    for row in route{for id in [row[0].as_u64()? as usize,row[1].as_u64()? as usize]{if limited(id){assigned.entry(id).or_insert(0);}}}
    let mut steps=Vec::new();
    for row in route{let a=row[0].as_u64()? as usize;let b=row[1].as_u64()? as usize;let child=row[2].as_u64()? as usize;
        let first=match strv(&row[3]){"M"=>0,"F"=>1,_=>assigned.get(&a).copied().unwrap_or_else(||assigned.get(&b).map(|s|1-s).unwrap_or(0))};
        let parent=|id,sex|offspring_parent(id,sex,group(id).and_then(|g|g.single.as_ref().or(g.parents[sex].as_ref())));
        steps.push(json!({"child":child,"depth":structural["bredDepth"][child],"parents":[parent(a,first),parent(b,1-first)]}));
    }Some(steps)
}
/// Bulk offspring calculation: exact assignments for <=6 unknown species,
/// structural witnesses plus bounded consistency search for larger collections.
pub fn offspring_solve(request:&Value)->Value{
    let n=request["speciesCount"].as_u64().unwrap_or(0) as usize;let stock=arr(&request["stock"]);let empty=||json!({"items":[],"unresolved":0});
    if n==0||n>100_000||stock.len()<2{return empty()}let Ok(pairs)=parse_pairs(request,n)else{return empty()};
    let eligible:HashSet<usize>=pairs.iter().flat_map(|p|[p.a,p.b]).collect();let usable:Vec<_>=stock.iter().filter(|p|p["species"].as_u64().is_some_and(|s|eligible.contains(&(s as usize)))).collect();
    if usable.len()<2||usable.iter().all(|p|p["sex"]=="M")||usable.iter().all(|p|p["sex"]=="F"){return empty()}
    let groups=parent_groups(stock);let mut parents=[vec![None;n],vec![None;n]];let mut unassigned=Vec::new();
    for (index,g) in &groups{if !eligible.contains(index){continue}for sex in 0..2{parents[sex][*index]=g.parents[sex].clone()}if let Some(single)=&g.single{unassigned.push(single.clone())}}
    let mut try_assignment=|items:&mut [Option<Value>],assign:&dyn Fn(usize)->usize|{
        for (i,r) in unassigned.iter().enumerate(){let s=assign(i);let species=r["species"].as_u64().unwrap() as usize;parents[s][species]=Some(r.clone());parents[1-s][species]=None;}
        offspring_fixed(&pairs,n,&parents,items);
    };
    let mut items=Vec::new();let mut unresolved=0;
    if unassigned.len()<=6{let mut all=vec![None;n];for mask in 0..(1usize<<unassigned.len()){try_assignment(&mut all,&|i|(mask>>i)&1)}items.extend(all.into_iter().flatten());}
    else{
        let structural=warehouse_solve(arr(&request["pairs"]),n,&groups.iter().map(|(id,_)|*id).collect::<Vec<_>>(),None);let mut samples:Option<Vec<Option<Value>>>=None;
        for target in 0..n{if structural["bredDepth"][target].is_null(){continue}
            let mut steps=validate_offspring_route(&warehouse_route(&structural,target,true),&structural,&groups);let mut minimum=true;
            if steps.is_none(){
                if samples.is_none(){let mut all=vec![None;n];for bit in 0..3{for flip in 0..2{try_assignment(&mut all,&|i|((i>>bit)&1)^flip)}}samples=Some(all)}
                let candidate=samples.as_ref().unwrap()[target].as_ref();
                if candidate.is_some_and(|c|c["depth"]==structural["bredDepth"][target]){items.push(candidate.unwrap().clone());continue}
                let mut req=request.clone();req["target"]=json!(target);req["traits"]=json!([]);req["allowRandom"]=json!(false);req["requireHatch"]=json!(true);let exact=trait_solve(&req);
                if exact["status"]!="ok"{if exact["status"]=="unconfirmed-sex"{if let Some(c)=candidate{let mut c=c.clone();c["minimumCertain"]=json!(false);items.push(c)}else{unresolved+=1}}continue}
                let founders:HashMap<String,&Value>=arr(&exact["founders"]).iter().map(|f|(strv(&f["id"]).into(),f)).collect();
                let mut exact_steps:Vec<Value>=arr(&exact["steps"]).iter().map(|s|{
                    let parents:Vec<Value>=arr(&s["parents"]).iter().map(|p|{let f=founders.get(strv(&p["id"]));json!({"species":p["species"],"sex":p["sex"],"stockId":f.map(|f|f["stockId"].clone()).unwrap_or(Value::Null),"unknown":f.is_some_and(|f|f["sex"]=="unknown")})}).collect();
                    json!({"child":s["species"],"depth":s["depth"],"parents":parents})
                }).collect();
                minimum=exact["minimumUnconfirmed"]!=true;
                if !minimum{if let Some(c)=candidate{if c["depth"].as_u64().unwrap()<exact_steps.last().unwrap()["depth"].as_u64().unwrap(){exact_steps=arr(&c["steps"]).to_vec();}}}steps=Some(exact_steps);
            }
            let steps=steps.unwrap();if let Some(last)=steps.last(){items.push(json!({"id":target,"depth":last["depth"],"minimumCertain":minimum,"conditional":steps.iter().any(|s|arr(&s["parents"]).iter().any(|p|p["unknown"]==true)),"steps":steps}));}
        }
    }
    items.sort_by_key(|v|(v["depth"].as_u64().unwrap_or(u64::MAX),v["id"].as_u64().unwrap_or(u64::MAX)));json!({"items":items,"unresolved":unresolved})
}
