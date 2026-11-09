// Palworld UI textures T_icon_skillstatus_rank_arrow_01…05 from PalDB's Passive_Skills page.
let passiveNameOrder;
function comparePassiveTraits(a,b) {
  const tier=b.rank-a.rank;
  if(tier) return tier;
  passiveNameOrder??=new Intl.Collator('zh-CN',{numeric:true});
  return passiveNameOrder.compare(a.zh,b.zh);
}

function passiveRankIcon(rank) {
  const texture=passiveRankTextures[Math.abs(rank)];
  if(!texture) return '';
  const label='词条等级 '+rank;
  return '<span class="passive-emblem" data-passive-rank="'+rank+'" title="'+label+'"><img width="16" height="16" alt="'+label+'" src="'+texture+'"></span>';
}
const passiveRankTextures = ["","data:image/webp;base64,UklGRtQAAABXRUJQVlA4WAoAAAAQAAAAFwAAFwAAQUxQSF4AAAABYBPbVpz/Z+iwgQoUoIDEB4OLrGI2C/RU6KBl9ixV0kbEBMAvRES8QkR8Iq2Vb2gbY4xWP2CDj5d+YFf8DA9P3oqhuQmxNUK8tTIAALY+cIrCJVXuynQEbpPetD4yVlA4IFAAAACQAwCdASoYABgAPmUmkEWjhv9VABABkS0gAAt2rfxscreBMVAAAP7+zL5/67/Tf/mX+oTn6n/+lMsvtynvaL/5l/j/b+p//jyvFYPk2zwAAA==","data:image/webp;base64,UklGRugAAABXRUJQVlA4WAoAAAAQAAAAFwAAFwAAQUxQSJ4AAAABcFpr29N8qRL5+/QRKBMwQZpjARwyvVjKBAyAZQIUPnUBpvj+84m/pEdHxATAHzs+ncZPMIITEdEpMLSaowS5EpNRU+HkiMh57ro554iYOzJCRIz7AAD9GBGRVHe/Ccqmf1eVHojzopiD6JUykS0qIqJqwUCbHUnzyFRORmJqmimJmSMjMewAAHRCElWXbQuU7d1VVc5aoN2el7IfDVZQOCAkAAAAcAIAnQEqGAAYAD5tNJZHgoCACADYlpAAA9o6AAD++yGAAAAA","data:image/webp;base64,UklGRjQBAABXRUJQVlA4WAoAAAAQAAAAFwAAFwAAQUxQSOoAAAABgBttf7vmx52kWkMQRO2ca0KNvQSnBWjtiswCjhMQBnD2CNk1K/yU7n7H749jHxEK27ZtmNlxCfzBaDw9NZbJUTRfIsVLM0eRodDDyAJ7BW5zjEimlcqUII43FyUkgzwAQH6AiJgY/2wVwNHd3SGAss+eeagrpVrzlFKat5RS9Qf2oMojn1j4UVmx2p55T85WV06J97Ntdh0o2hq0Bt0OFIl7sRJ0+/OzrUHaF+6uLqVpp4VoGynrd+xBmlHgX8LISFYHsxDIlSldkRBm+5lfTgwAQOl4yZdnK4CFsM/cdU2ILELUruEfBABWUDggJAAAAHACAJ0BKhgAGAA+bTSWR4KAgAgA2JaQAAPaOgAA/vshgAAAAA==","data:image/webp;base64,UklGRnIBAABXRUJQVlA4WAoAAAAQAAAAFwAAFwAAQUxQSPgAAAABgGLbttvmviWc4jaD9MZ7Yz3MAxBPTwYgnva5qSeQynoyAncoaB721l8X6L9YQ4iICQAAERGYIiIwQ9Vms5rHVZMkm1cDjF3/0fy7HrOWXpl9mF94YPZ1KUSSqZsHgAWXkqSVxkPAQaNxCAzFqfUbA0MnDZJsnA4B8W8I0YRTmuomIoRXn1SVrC8u1klVfVoNUVU1qQBAJVFVpdU9ilBJyKSC6KhrfR0C084H3TRw+BUCpmv0JmvTMLe+SXrvxsec9yS/N0Mk2XfjIsCY65Ok4XtHAqBQACBHPW+8bYtkyIzI9lsIkikUyEIhI/gfgwMEjVxJ5JzfgFZQOCBUAAAAEAMAnQEqGAAYAD5tLJJFpIYBABABsS0gAAlMi0c9mKwAAP77qlNP/7/87g//Qz/ko8qt8i4r9T8/DDgh/+//ncH/6Gf0DL2ffFLWeiKoQypUgAAA","data:image/webp;base64,UklGRpoBAABXRUJQVlA4WAoAAAAQAAAAFwAAFwAAQUxQSCABAAABkGrb1rJl+33HVxkkuIJP2ptxd21kp7l0iw6VC4DqkNwuwF0TmcYJC/hfG/8VRMQESJLjOI4CHcdxFOjXdnnZFsfYJcDlWITk9BeBX9PJoLw1/l33vHX+3c3z499FT5K8xQ+AoPv2bKn14qJVyur6CDpvl7IHLwAuBrOkrnM/JTKLRqAtZhLyL9g0M1gpyF8BM9ss8MPMbCpXknImzcwIuu1PKHcKJnOU6L8NOm2RMuPfYGbzGanl1E/KLPOvmRnLGQWWHgF8r3rJDTPgqMQP4HPck6Tk/CdA0EO/KxXu7hRL6nsIOmpwXbdwF2CnWFL9kZ9ct2EH3np6X2G7XiHdfmBXknaApRBSxSE8tXc+wUm1IpbvAexVKsbyvb1K/YdWUDggVAAAABADAJ0BKhgAGAA+bSySRaSGAQAQAbEtIAAJTItHPZisAAD++6pTT/+//O4P/0M/5KPKrfIuK/U/Pww4If/v/53B/+hn9Ay9n3xS1noiqEMqVIAAAA=="];
