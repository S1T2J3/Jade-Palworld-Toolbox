"""Build offline effects by exact game item ID; no runtime scraping or AI guesses.

Inputs: the existing item-data extraction plus fetch-effects.py's pinned tables.
Usage: python tools/build-item-effects.py --source ../item-data
"""
import argparse, hashlib, json, re
from pathlib import Path
from urllib.parse import unquote
from urllib.request import Request, urlopen
from lxml import html

base = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--source', type=Path, default=base.parent/'item-data')
parser.add_argument('--download', action='store_true', help='Fetch absent inputs; never replaces cached evidence')
args = parser.parse_args()
source = args.source
if args.download:
    source.mkdir(parents=True, exist_ok=True)
    pinned = 'https://raw.githubusercontent.com/Dhampyru/Palworld-Extracted/d38105b4dc0cf6f36c5035997fbe54205bb2d753/Pal/Content/Pal/DataTable/'
    inputs = {'DT_ItemDataTable_Common.json':pinned+'Item/DT_ItemDataTable_Common.json',
        'effects-passives.json':pinned+'PassiveSkill/DT_PassiveSkill_Main_Common.json',
        'effects-food.json':pinned+'Item/DT_StatusEffectFood.json',
        'effects-status.json':pinned+'Item/DT_GainStatusPointsItem.json',
        'effects-items-cn.html':'https://paldb.cc/cn/Items'}
    for name,url in inputs.items():
        if not (source/name).exists():
            (source/name).write_bytes(urlopen(Request(url,headers={'User-Agent':'Jade-Palworld-Toolbox'}),timeout=30).read())
def rows(name):
    return json.loads((source/name).read_text(encoding='utf8'))[0]['Rows']
raw = rows('DT_ItemDataTable_Common.json')
passives = rows('effects-passives.json')
foods = rows('effects-food.json')
status = rows('effects-status.json')
page = html.fromstring((source/'effects-items-cn.html').read_text(encoding='utf8'))
catalog = json.loads((base/'data/item-catalog-data.json').read_text(encoding='utf8'))
names = {i['id']:i['name'] for i in catalog['items']}
extra_path = base/'crates/jade-app/assets/item-effects-extra.json'
extra = json.loads(extra_path.read_text(encoding='utf8'))
details = json.loads((base/'data/pal-details-data.json').read_text(encoding='utf8'))
capture = json.loads((base/'data/capture-data.json').read_text(encoding='utf8'))
sphere_power = {s['id']:s['power'] for s in capture['spheres']}
implants = {p['surgery']['item']:dict(key=k,name=p['zh'],effect=p['effect'])
    for k,p in details['passives'].items() if isinstance(p.get('surgery'),dict) and p['surgery'].get('item')}
assert set(names) == set(raw), 'Item versions differ; reconcile IDs before building'
passive_names = {unquote(a.get('data-hover')).split('/',1)[-1]:a.text_content().strip()
    for a in page.xpath('//*[@data-hover and contains(@data-hover,"PassiveSkills%2F")]')}
descriptions = {}
descriptions_by_slug = {}
references = {}
for a in page.xpath('//div[contains(@class,"flex-grow-1")]/a[@data-hover]'):
    hover = unquote(a.get('data-hover'))
    if not hover.startswith('?s=Items/'): continue
    key = hover.split('/',1)[1]
    bodies = a.xpath('following-sibling::div[1]')
    body = bodies[0] if bodies else None
    if body is None: continue
    # Exclude passive badges from the narrative; they appear in structured effects.
    copy = html.fromstring(html.tostring(body,encoding='unicode'))
    for badge in copy.xpath('.//*[contains(@class,"item_skill_bar")]'): badge.drop_tree()
    descriptions[key] = re.sub(r'\s+', ' ', copy.text_content()).strip()
    if descriptions[key]: descriptions_by_slug[a.get('href')] = descriptions[key]
    references[key] = list(dict.fromkeys(unquote(n.get('data-hover')).split('/',1)[1]
        for n in body.xpath('.//a[@data-hover]') if unquote(n.get('data-hover')).startswith('?s=Items/')
        and unquote(n.get('data-hover')).split('/',1)[1] != key))

elements = {'Normal':'无','Fire':'火','Water':'水','Leaf':'草','Electricity':'雷','Ice':'冰','Earth':'地','Dark':'暗','Dragon':'龙'}
percent = {'ShotAttack':'攻击力','Defense':'防御力','MaxHP':'生命值上限','CraftSpeed':'工作速度',
    'PalExp_Increase':'帕鲁经验获取量','FriendshipPoint_Increase':'帕鲁信赖度上升量','LifeSteal':'攻击吸血',
    'ExplosionResist':'爆炸伤害减免'}
flat = {'MaxInventoryWeight':'负重上限','CaptureLevel':'捕获力等级','JumpCount_Increase':'额外连续跳跃次数','AirDash':'空中冲刺次数'}
def passive_effect(key):
    v = passives[key]
    title = passive_names.get(key, '')
    parts = []
    for n in range(1,5):
        kind = v[f'EffectType{n}'].split('::')[-1]
        value = v[f'EffectValue{n}']
        target = '队伍帕鲁' if v[f'TargetType{n}'].endswith('::ToOtomo') else '自身'
        if kind in percent:
            parts.append(f'{target}{percent[kind]} +{value:g}%')
        elif kind in flat:
            parts.append(f'{target}{flat[kind]} +{value:g}')
        elif kind.startswith('TemperatureResist_'):
            parts.append(('耐寒' if kind.endswith('Cold') else '耐暑')+f' Lv{value:g}')
        elif kind.startswith('ElementBoost_'):
            parts.append(f'{target}{elements[kind.split("_")[1]]}属性伤害 +{value:g}%')
        elif kind.startswith('ElementResist_'):
            parts.append(f'{target}受到的{elements[kind.split("_")[1]]}属性伤害 -{value:g}%')
        elif kind == 'CollectItemDrop_NaturalObject':
            label = '采矿' if key.startswith('StonDrop') else '伐木' if key.startswith('WoodDrop') else '采矿／伐木'
            parts.append(f'{label}采集量 +{value:g}%')
        elif kind == 'NonKilling': parts.append('攻击后至少为目标留下 1 点生命值')
        elif kind == 'InvalidToxicGas': parts.append(f'{target}免疫腐雾伤害')
        # Enum values (e.g. CurveType=19) are not percentages or levels.
        # Unknown/special units retain the localized game effect name.
    if not title and not parts: return '特殊效果：未收录可核实的说明'
    detail = '；'.join(dict.fromkeys(parts))
    return f'{title} · {detail}' if title and detail and re.sub(r'\s+', '', title) != re.sub(r'\s+', '', detail) else title or detail

fields = [('PhysicalAttackValue','攻击力'),('PhysicalDefenseValue','防御力'),('HPValue','生命值'),
    ('ShieldValue','护盾值'),('Durability','耐久度'),('MagazineSize','弹匣容量'),
    ('RestoreSatiety','营养'),('RestoreSanity','SAN恢复'),('RestoreHealth','生命恢复')]
result = {}
for key,v in raw.items():
    kind = v['TypeA'].split('::')[-1]
    stats = [[label,f'{v[field]:g}'] for field,label in fields if v.get(field,0)>0]
    if v.get('ElementType','').split('::')[-1] in elements:
        stats.append(['属性',elements[v['ElementType'].split('::')[-1]]])
    stats += [['重量',f'{v["Weight"]:g}'],['堆叠上限',str(v['MaxStackCount'])]]
    if key in sphere_power: stats.insert(0,['捕获力',str(sphere_power[key])])
    effects = [passive_effect(v[field]) for field in ['PassiveSkillName','PassiveSkillName2','PassiveSkillName3','PassiveSkillName4'] if v[field]!='None']
    food = foods.get(key)
    if food and food['EffectTime']:
        labels = {'Attack':'攻击力','Defense':'防御力','WorkSpeed':'工作速度','Exp_Increase':'经验获取量',
            'HungerResist':'饥饿下降减缓','SANResist':'SAN下降减缓','ExplosionResist':'爆炸伤害减免'}
        for n in [1,2]:
            t = food[f'EffectType{n}'].split('::')[-1]; value = food[f'EffectValue{n}']; interval = food.get(f'Interaval{n}',0)
            if t in labels: effects.append(f'{labels[t]} +{value:g}% · 持续 {food["EffectTime"]:g} 秒')
            elif t=='Regene_Hp': effects.append(f'持续恢复生命 · 持续 {food["EffectTime"]:g} 秒')
            elif t=='LeanBackAndKnockbackInvalid': effects.append(f'免疫后仰与击退 · 持续 {food["EffectTime"]:g} 秒')
            elif t=='FullStomachKeep': effects.append(f'延缓饥饿 · 持续 {food["EffectTime"]:g} 秒')
    for field,label in [('MaxHP','生命值'),('MaxSP','体力'),('Power','攻击'),('WorkSpeed','工作速度'),('MaxInventoryWeight','负重')]:
        if status.get(key,{}).get(field): effects.append(f'永久增加{label}属性点 +{status[key][field]:g}')
    move = v.get('WazaID','').split('::')[-1]
    skill = move if move not in ['None',''] else ''
    desc = descriptions.get(key,'')
    if not desc:
        item = next(i for i in catalog['items'] if i['id']==key)
        desc = descriptions_by_slug.get(item.get('slug',''),'')
    if not desc: desc = extra.get(key,{}).get('description','')
    if key in implants:
        p = implants[key]
        desc = desc or f'在帕鲁手术台使用，为帕鲁添加「{p["name"]}」被动技能。'
        effects.append(p['effect'].replace('\n','；'))
    if not desc and not effects and not skill:
        if kind=='Blueprint': desc='用于对应装备或物品的制造；配方与制作设施见下方合成表。'
        elif kind=='Material': desc='制作与兑换材料；可用于下方列出的合成配方和关联条目。'
        elif not v.get('bLegalInGame'): desc='游戏数据保留条目，尚无可核实的使用效果。'
        else: desc='此物品的特殊使用效果尚未收录。'
    result[key] = dict(description=desc,stats=stats,effects=effects,skill=skill,
        passive=implants.get(key,{}),
        relatedItems=[k for k in references.get(key,[]) if k in names])

files=['DT_ItemDataTable_Common.json','effects-passives.json','effects-food.json','effects-status.json','effects-items-cn.html']
output = dict(schema=1, sourceCommit='d38105b4dc0cf6f36c5035997fbe54205bb2d753',
    sources=['https://paldb.cc/cn/Items','https://github.com/Dhampyru/Palworld-Extracted/tree/d38105b4dc0cf6f36c5035997fbe54205bb2d753/Pal/Content/Pal/DataTable'],
    inputSha256={name:hashlib.sha256((source/name).read_bytes()).hexdigest() for name in files},items=result)
output['supplementSources'] = [row['source'] for row in extra.values()]
output['inputSha256']['item-effects-extra.json']=hashlib.sha256(extra_path.read_bytes()).hexdigest()
assert result['StealArmor']['stats'][:3]==[['防御力','220'],['生命值','750'],['耐久度','5000']]
assert ['防御力','352'] in result['StealArmor_5']['stats']
assert any('+15%' in e for e in result['Accessory_AT_1']['effects'])
assert any('30%' in e and '600 秒' in e for e in result['Salad']['effects'])
assert result['SkillCard_AquaJet']['skill']=='AquaJet'
assert result['PalSphere']['stats'][0]==['捕获力','7']
assert any('5%' in e for e in result['PalPassiveSkillChange_Consumable_Vampire']['effects'])
dest = base/'crates/jade-app/assets/item-effects-data.json'
dest.write_text(json.dumps(output, ensure_ascii=False,separators=(',',':')),encoding='utf8')
(base/'data/item-effects-data.json').write_bytes(dest.read_bytes())
print(json.dumps(dict(items=len(result),descriptions=len(descriptions),equipment=sum(v['TypeA'].split('::')[-1] in ['Weapon','Armor','Accessory','Glider','CaptureItemModifier','SpecialWeapon'] for v in raw.values()),passiveItems=sum(bool(v['effects']) for v in result.values()),bytes=dest.stat().st_size),ensure_ascii=False))
