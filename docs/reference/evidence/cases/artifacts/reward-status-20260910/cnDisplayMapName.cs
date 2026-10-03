using System;
using System.Collections;
using UnityEngine;

public class cnDisplayMapName : MonoBehaviour
{
	private float fStartTime;

	private string mapName;

	public GUISkin pMenuSkin;

	public float fMaxtime;

	private int iMovetype;

	public Texture CrateTexture;

	public Texture BoostTexture;

	public Texture PotionTexture;

	public Texture ShinyTexture;

	public Texture FullInvenTex;

	public Texture TarosTexture;

	public Texture FMTexture;

	public Rect RaceRingRect;

	public Rect RaceTimeRect;

	public float fRewardIconTime;

	private cnOwnAvatarStatus ownstatus;

	private int iCurTaros;

	private int iCurFM;

	private int iTargetTaros;

	private int iTargetFM;

	private float fLastUpTime;

	private bool bFullInven;

	private float fFullTimer;

	private float fcheckTimer;

	private ArrayList RewardCrate;

	private ArrayList RewardBoost;

	private ArrayList RewardPotion;

	private ArrayList ShinyBuffIcon;

	private ArrayList QuestItemList;

	private bool bRenderName;

	private UserSlot pUserSlot;

	private cnMissionManager missionManager;

	private float fFM_Timer;

	private float fTaros_Timer;

	private GUIStyle MenuCenterBox3;

	private GUIStyle MenuBigFont16;

	public cnDisplayMapName()
	{
		//IL_0028: Unknown result type (might be due to invalid IL or missing references)
		//IL_002d: Unknown result type (might be due to invalid IL or missing references)
		//IL_0047: Unknown result type (might be due to invalid IL or missing references)
		//IL_004c: Unknown result type (might be due to invalid IL or missing references)
		fMaxtime = 2f;
		RaceRingRect = new Rect(0f, 0f, 200f, 50f);
		RaceTimeRect = new Rect(0f, 0f, 200f, 50f);
		fRewardIconTime = 2f;
		RewardCrate = new ArrayList();
		RewardBoost = new ArrayList();
		RewardPotion = new ArrayList();
		ShinyBuffIcon = new ArrayList();
		QuestItemList = new ArrayList();
	}

	private void Start()
	{
		MenuCenterBox3 = pMenuSkin.GetStyle("centerbox3");
		MenuBigFont16 = pMenuSkin.GetStyle("BigFont16");
		pUserSlot = Object.FindObjectOfType(typeof(UserSlot)) as UserSlot;
		fcheckTimer = Time.get_time();
		GameObject manager = GlobalManager.GetManager(12);
		missionManager = manager.GetComponent(typeof(cnMissionManager)) as cnMissionManager;
	}

	private void Update()
	{
		if (Time.get_time() - fStartTime > fMaxtime)
		{
			bRenderName = false;
		}
		if (bFullInven && fFullTimer > 0f)
		{
			fFullTimer -= Time.get_deltaTime();
		}
		if (RewardCrate.Count > 0)
		{
			((RewardDisplay)RewardCrate[0]).fDisplayTime -= Time.get_deltaTime();
			if (((RewardDisplay)RewardCrate[0]).fDisplayTime < 0f - fRewardIconTime)
			{
				RewardCrate.RemoveAt(0);
			}
		}
		else if (Object.op_Implicit((Object)(object)pUserSlot) && Time.get_time() - fcheckTimer > 1.5f)
		{
			fcheckTimer = Time.get_time();
			bool emptyInventory = pUserSlot.GetEmptyInventory();
			if (emptyInventory == bFullInven && !bFullInven)
			{
				fFullTimer = fRewardIconTime;
			}
			bFullInven = !emptyInventory;
		}
		if (RewardBoost.Count > 0)
		{
			((RewardDisplay)RewardBoost[0]).fDisplayTime -= Time.get_deltaTime();
			if (((RewardDisplay)RewardBoost[0]).fDisplayTime < 0f - fRewardIconTime)
			{
				RewardBoost.RemoveAt(0);
			}
		}
		if (RewardPotion.Count > 0)
		{
			((RewardDisplay)RewardPotion[0]).fDisplayTime -= Time.get_deltaTime();
			if (((RewardDisplay)RewardPotion[0]).fDisplayTime < 0f - fRewardIconTime)
			{
				RewardPotion.RemoveAt(0);
			}
		}
		if (ShinyBuffIcon.Count > 0)
		{
			((RewardDisplay)ShinyBuffIcon[0]).fDisplayTime -= Time.get_deltaTime();
			if (((RewardDisplay)ShinyBuffIcon[0]).fDisplayTime < 0f - fRewardIconTime)
			{
				ShinyBuffIcon.RemoveAt(0);
			}
		}
		if (QuestItemList.Count > 0)
		{
			((RewardDisplay)QuestItemList[0]).fDisplayTime -= Time.get_deltaTime();
			if (((RewardDisplay)QuestItemList[0]).fDisplayTime <= 0f)
			{
				QuestItemList.RemoveAt(0);
			}
		}
		bool flag = false;
		if (Time.get_time() - fLastUpTime > 0.1f)
		{
			if (iCurFM != iTargetFM)
			{
				if (iCurFM < iTargetFM)
				{
					int num = Mathf.Clamp((iTargetFM - iCurFM) / 5, 1, 10);
					iCurFM += num;
				}
				else
				{
					int num2 = Mathf.Clamp((iCurFM - iTargetFM) / 5, 1, 10);
					iCurFM -= num2;
				}
				flag = true;
			}
			else if (fFM_Timer > 0f)
			{
				fFM_Timer -= Time.get_deltaTime();
			}
			if (iCurTaros != iTargetTaros)
			{
				if (iCurTaros < iTargetTaros)
				{
					int num3 = Mathf.Clamp((iTargetTaros - iCurTaros) / 5, 1, 10);
					iCurTaros += num3;
				}
				else
				{
					int num4 = Mathf.Clamp((iCurTaros - iTargetTaros) / 5, 1, 10);
					iCurTaros -= num4;
				}
				flag = true;
			}
			else if (fTaros_Timer > 0f)
			{
				fTaros_Timer -= Time.get_deltaTime();
			}
		}
		if (flag)
		{
			fLastUpTime = Time.get_time();
		}
	}

	public void SetAvatar(Transform user)
	{
		ownstatus = ((Component)user).GetComponent(typeof(cnOwnAvatarStatus)) as cnOwnAvatarStatus;
		iCurTaros = ownstatus.iTaros;
		iCurFM = ownstatus.iFusionMatter;
		iTargetFM = iCurFM;
		iTargetTaros = iCurTaros;
		ClearRewardIcon();
	}

	public void ClearRewardIcon()
	{
		RewardCrate.Clear();
		RewardBoost.Clear();
		RewardPotion.Clear();
		ShinyBuffIcon.Clear();
	}

	public void QuestItem(int iMobType, int iQuestItemID, int iTaskID)
	{
		//IL_0035: Unknown result type (might be due to invalid IL or missing references)
		//IL_003b: Expected O, but got Unknown
		//IL_0049: Unknown result type (might be due to invalid IL or missing references)
		//IL_0050: Expected O, but got Unknown
		//IL_0059: Unknown result type (might be due to invalid IL or missing references)
		//IL_0060: Expected O, but got Unknown
		//IL_006f: Unknown result type (might be due to invalid IL or missing references)
		//IL_0076: Expected O, but got Unknown
		//IL_017a: Unknown result type (might be due to invalid IL or missing references)
		//IL_0181: Expected O, but got Unknown
		//IL_0190: Unknown result type (might be due to invalid IL or missing references)
		//IL_0197: Expected O, but got Unknown
		//IL_01a0: Unknown result type (might be due to invalid IL or missing references)
		//IL_01a7: Expected O, but got Unknown
		//IL_01b6: Unknown result type (might be due to invalid IL or missing references)
		//IL_01bd: Expected O, but got Unknown
		if (iQuestItemID > 0)
		{
			RewardDisplay rewardDisplay = new RewardDisplay();
			rewardDisplay.iCount = 1;
			rewardDisplay.fDisplayTime = fRewardIconTime * 1.5f;
			QuestItemElement val = (QuestItemElement)TableContainer.GetTableData(29, 0, iQuestItemID);
			StringTableElement val2 = (StringTableElement)TableContainer.GetTableData(29, 1, val.m_iItemName);
			NpcTableElement val3 = (NpcTableElement)TableContainer.GetTableData(10, 0, iMobType);
			StringTableElement val4 = (StringTableElement)TableContainer.GetTableData(10, 1, val3.m_iNpcName);
			string str = TextManager.GetStr("the %s had: %s!", val4.m_strName, val2.m_strName);
			rewardDisplay.title = str.ToUpper();
			QuestItemList.Add(rewardDisplay);
			return;
		}
		int count = missionManager.m_ActivateMissionList.Count;
		for (int i = 0; i < count; i++)
		{
			MissionElement me = ((cnMissionNode)missionManager.m_ActivateMissionList[i]).GetMe();
			if (me == null || me.m_iHTaskID != iTaskID)
			{
				continue;
			}
			for (int j = 0; j < 3; j++)
			{
				if (me.m_iCSUEnemyID[j] == iMobType && me.m_iCSUNumToKill[j] == 0)
				{
					iQuestItemID = me.m_iSTItemID[j];
					RewardDisplay rewardDisplay2 = new RewardDisplay();
					rewardDisplay2.iCount = -1;
					rewardDisplay2.fDisplayTime = fRewardIconTime * 1.5f;
					QuestItemElement val5 = (QuestItemElement)TableContainer.GetTableData(29, 0, iQuestItemID);
					StringTableElement val6 = (StringTableElement)TableContainer.GetTableData(29, 1, val5.m_iItemName);
					NpcTableElement val7 = (NpcTableElement)TableContainer.GetTableData(10, 0, iMobType);
					StringTableElement val8 = (StringTableElement)TableContainer.GetTableData(10, 1, val7.m_iNpcName);
					string str2 = TextManager.GetStr("the %s did not have: %s.", val8.m_strName, val6.m_strName);
					rewardDisplay2.title = str2.ToUpper();
					QuestItemList.Add(rewardDisplay2);
					return;
				}
			}
		}
	}

	public void ShinyIcon(sP_FE2CL_REP_SHINY_PICKUP_SUCC succ)
	{
		//IL_0008: Unknown result type (might be due to invalid IL or missing references)
		//IL_000e: Expected O, but got Unknown
		SkillTableScript val = (SkillTableScript)TableContainer.GetTable(12);
		RewardDisplay rewardDisplay = new RewardDisplay();
		rewardDisplay.title = val.m_pSkillStringData[succ.iSkillID].m_strName;
		CnGuiChat.AddEventString(5, TextManager.GetStr("You receive <%s> from Coco's Egg", rewardDisplay.title));
		if (succ.eCSTB == 0)
		{
			SkillElement val2 = val.m_pSkillData[succ.iSkillID];
			IconElement pIconTable = val.m_pSkillIconData[val2.m_iIcon];
			rewardDisplay.Icon = AvatarUtil.GetIconTexture(pIconTable);
			rewardDisplay.fDisplayTime = fRewardIconTime;
			ShinyBuffIcon.Add(rewardDisplay);
		}
		else
		{
			IconElement pIconTable2 = val.m_pSkillIconData[val.m_pSkillBuffData[succ.eCSTB].m_iBuffIcon];
			rewardDisplay.Icon = AvatarUtil.GetIconTexture(pIconTable2);
			rewardDisplay.fDisplayTime = fRewardIconTime;
			ShinyBuffIcon.Add(rewardDisplay);
		}
	}

	public void RewardItem(sItemReward item)
	{
		//IL_0097: Unknown result type (might be due to invalid IL or missing references)
		//IL_009d: Expected O, but got Unknown
		if (!cntutorialscript.bTutorial && item.eIL == 1 && item.sItem.iType == 9)
		{
			cnEvent cnEvent2 = new cnEvent(1, 1);
			cnEvent2[0] = 28;
			cnEvent2[1] = 0;
			cnEvent2[2] = (int)item.sItem.iID;
			Logger.LogTrace("item.sitem.iid " + item.sItem.iID);
			cnEvent.SendEvent(cnEvent2);
			ChestItemElement val = (ChestItemElement)cnEvent2[0];
			RewardDisplay rewardDisplay = new RewardDisplay();
			rewardDisplay.fDisplayTime = fRewardIconTime;
			rewardDisplay.iCount = 1;
			if (val.m_iChestCheck == 0)
			{
				rewardDisplay.title = TextManager.GetStr("+1 C.R.A.T.E");
				CnGuiChat.AddEventString(5, TextManager.GetStr("You receive 1 C.R.A.T.E"));
				RewardCrate.Add(rewardDisplay);
			}
			else if (val.m_iChestCheck == 1)
			{
				rewardDisplay.title = TextManager.GetStr("+1 E.G.G");
				CnGuiChat.AddEventString(5, TextManager.GetStr("You receive 1 E.G.G"));
				cnFirstUseSysManager.CheckCondition(68);
			}
		}
	}

	public void RewardFull()
	{
		if (!cntutorialscript.bTutorial)
		{
			fFullTimer = fRewardIconTime;
			bFullInven = true;
			CnGuiChat.AddEventString(5, TextManager.GetStr("Inventory Full") + ".");
		}
	}

	public void RewardFM(sP_FE2CL_REP_REWARD_ITEM reward)
	{
		if (!cntutorialscript.bTutorial)
		{
			cnOwnAvatarStatus cnOwnAvatarStatus2 = ((Component)UserContainer.GetPlayer()).GetComponent(typeof(cnOwnAvatarStatus)) as cnOwnAvatarStatus;
			if (cnOwnAvatarStatus2.iBatteryNano < reward.m_iBatteryN)
			{
				RewardDisplay rewardDisplay = new RewardDisplay();
				rewardDisplay.fDisplayTime = fRewardIconTime;
				rewardDisplay.iCount = Mathf.Max(0, reward.m_iBatteryN - cnOwnAvatarStatus2.iBatteryNano);
				rewardDisplay.title = "+" + rewardDisplay.iCount + " " + TextManager.GetStr("POTIONS");
				RewardPotion.Add(rewardDisplay);
			}
			if (cnOwnAvatarStatus2.iBatteryWpn < reward.m_iBatteryW)
			{
				RewardDisplay rewardDisplay2 = new RewardDisplay();
				rewardDisplay2.fDisplayTime = fRewardIconTime;
				rewardDisplay2.iCount = Mathf.Max(0, reward.m_iBatteryW - cnOwnAvatarStatus2.iBatteryWpn);
				rewardDisplay2.title = "+" + rewardDisplay2.iCount + " " + TextManager.GetStr("BOOSTS");
				RewardBoost.Add(rewardDisplay2);
			}
			if (iCurFM > ownstatus.iFusionMatter)
			{
				iCurFM = ownstatus.iFusionMatter;
			}
			if (iCurTaros > ownstatus.iTaros)
			{
				iCurTaros = ownstatus.iTaros;
			}
			iTargetFM = reward.m_iFusionMatter;
			iTargetTaros = reward.m_iCandy;
			if (ownstatus.iTaros < iTargetTaros)
			{
				CnGuiChat.AddEventString(5, TextManager.GetStr("You earned %s Taros for a total %s taros.", (iTargetTaros - ownstatus.iTaros).ToString(), iTargetTaros.ToString()));
				SoundUtil.Playsound("Taros");
				fTaros_Timer = 2f;
			}
			if (ownstatus.iFusionMatter < iTargetFM)
			{
				CnGuiChat.AddEventString(5, TextManager.GetStr("You collected %s Fusion Matter for a total %s Fusion Matter.", (iTargetFM - ownstatus.iFusionMatter).ToString(), iTargetFM.ToString()));
				SoundUtil.Playsound("FusionMatter");
				fFM_Timer = 2f;
			}
		}
	}

	public void ReceiveNewMapName(cnEvent myevent)
	{
		if (!cntutorialscript.bTutorial)
		{
			cnEvent cnEvent2 = new cnEvent(2, 2);
			cnEvent.SendEvent(cnEvent2);
			if ((int)cnEvent2[0] == 5 || cnEvent2.GetCount() > 1)
			{
				bRenderName = true;
				mapName = (string)myevent[0];
				fStartTime = Time.get_time();
				iMovetype = Random.Range(0, 8);
			}
		}
	}

	public void ReceiveGeneralMessage(cnEvent myevent)
	{
		bRenderName = true;
		mapName = (string)myevent[0];
		fStartTime = Time.get_time();
		iMovetype = (int)myevent[1];
	}

	public void OnOwnGUI()
	{
		//IL_0024: Unknown result type (might be due to invalid IL or missing references)
		//IL_002a: Invalid comparison between Unknown and I4
		//IL_004c: Unknown result type (might be due to invalid IL or missing references)
		//IL_0051: Unknown result type (might be due to invalid IL or missing references)
		//IL_0057: Unknown result type (might be due to invalid IL or missing references)
		//IL_005d: Invalid comparison between Unknown and I4
		//IL_0106: Unknown result type (might be due to invalid IL or missing references)
		//IL_010b: Unknown result type (might be due to invalid IL or missing references)
		//IL_04ec: Unknown result type (might be due to invalid IL or missing references)
		//IL_04f4: Unknown result type (might be due to invalid IL or missing references)
		//IL_04f6: Unknown result type (might be due to invalid IL or missing references)
		//IL_052f: Unknown result type (might be due to invalid IL or missing references)
		//IL_0535: Unknown result type (might be due to invalid IL or missing references)
		//IL_0540: Unknown result type (might be due to invalid IL or missing references)
		//IL_0554: Unknown result type (might be due to invalid IL or missing references)
		//IL_055b: Unknown result type (might be due to invalid IL or missing references)
		//IL_056f: Unknown result type (might be due to invalid IL or missing references)
		if (cnSystemMessageManager.IsSysPopUp())
		{
			GUI.set_enabled(false);
		}
		else
		{
			GUI.set_enabled(true);
		}
		float num = 0f;
		if ((int)localized.quickSlot == 1)
		{
			num = 40f;
		}
		GUI.set_depth(12);
		GUI.set_skin(pMenuSkin);
		Color white = Color.get_white();
		if ((int)Event.get_current().get_type() != 7)
		{
			return;
		}
		if (!CnGuiChat.IsInputEnabled())
		{
			FFGUIUtility.ScaleAroundPivot((ScreenPivot)6);
			DoRewardGUI(num, ref white);
			FFGUIUtility.UnscaleAroundPivot((ScreenPivot)6);
		}
		if (bRenderName)
		{
			Rect val = default(Rect);
			((Rect)(ref val))..ctor(0f, 0f, (float)(mapName.Length * 16 + 20), 40f);
			float num2 = (Time.get_time() - fStartTime) / fMaxtime;
			num2 += Mathf.Sin(num2 * 2f * 3.14f) * 0.1f;
			white = Color.get_white();
			white.a = Mathf.Min(1f, (1f - num2) * 2f);
			switch (iMovetype)
			{
			case 0:
				((Rect)(ref val)).set_x((num2 + Mathf.Sin(num2 * 2f * 3.14f) * 0.15f) * (float)Screen.get_width() - (float)(mapName.Length * 8));
				((Rect)(ref val)).set_y((float)Screen.get_height() * 0.25f);
				break;
			case 1:
				((Rect)(ref val)).set_x((1f - (num2 + Mathf.Sin(num2 * 2f * 3.14f) * 0.15f)) * (float)Screen.get_width() - (float)(mapName.Length * 8));
				((Rect)(ref val)).set_y((float)Screen.get_height() * 0.25f);
				break;
			case 2:
				((Rect)(ref val)).set_x((float)Screen.get_width() * 0.5f - (float)(mapName.Length * 8));
				((Rect)(ref val)).set_y((num2 + Mathf.Sin(num2 * 2f * 3.14f) * 0.15f) * (float)Screen.get_height());
				break;
			case 3:
				((Rect)(ref val)).set_x((float)Screen.get_width() * 0.5f - (float)(mapName.Length * 8));
				((Rect)(ref val)).set_y((1f - (num2 + Mathf.Sin(num2 * 2f * 3.14f) * 0.15f)) * (float)Screen.get_height());
				break;
			case 4:
				((Rect)(ref val)).set_x((num2 + Mathf.Sin(num2 * 2f * 3.14f) * 0.15f) * (float)Screen.get_width() - (float)(mapName.Length * 8));
				((Rect)(ref val)).set_y((0.25f + Mathf.Sin(num2 * 6f * 3.14f) * Mathf.Abs(Mathf.Cos(num2 * 3.14f)) * 0.1f) * (float)Screen.get_height());
				break;
			case 5:
				((Rect)(ref val)).set_x((1f - (num2 + Mathf.Sin(num2 * 2f * 3.14f) * 0.15f)) * (float)Screen.get_width() - (float)(mapName.Length * 8));
				((Rect)(ref val)).set_y((0.25f + Mathf.Sin(num2 * 6f * 3.14f) * Mathf.Abs(Mathf.Cos(num2 * 3.14f)) * 0.1f) * (float)Screen.get_height());
				break;
			case 6:
				((Rect)(ref val)).set_x((0.5f + Mathf.Sin(num2 * 6f * 3.14f) * Mathf.Abs(Mathf.Cos(num2 * 3.14f)) * 0.1f) * (float)Screen.get_width() - (float)(mapName.Length * 8));
				((Rect)(ref val)).set_y((num2 + Mathf.Sin(num2 * 2f * 3.14f) * 0.15f) * (float)Screen.get_height());
				break;
			case 7:
				((Rect)(ref val)).set_x((0.5f + Mathf.Sin(num2 * 6f * 3.14f) * Mathf.Abs(Mathf.Cos(num2 * 3.14f)) * 0.1f) * (float)Screen.get_width() - (float)(mapName.Length * 8));
				((Rect)(ref val)).set_y((1f - (num2 + Mathf.Sin(num2 * 2f * 3.14f) * 0.15f)) * (float)Screen.get_height());
				break;
			}
			FFGUIUtility.ScaleAroundRect(val);
			Rect val2 = val;
			((Rect)(ref val2)).set_x(((Rect)(ref val2)).get_x() + 1f);
			((Rect)(ref val2)).set_y(((Rect)(ref val2)).get_y() + 1f);
			GUI.set_color(new Color(0f, 0f, 0f, white.a));
			GUI.Label(val2, mapName, MenuBigFont16);
			GUI.set_color(white);
			GUI.Label(val, mapName, MenuBigFont16);
			FFGUIUtility.UnscaleAroundRect(val);
		}
	}

	private void DoRewardGUI(float num, ref Color white)
	{
		//IL_005f: Unknown result type (might be due to invalid IL or missing references)
		//IL_0061: Unknown result type (might be due to invalid IL or missing references)
		//IL_011c: Unknown result type (might be due to invalid IL or missing references)
		//IL_0144: Unknown result type (might be due to invalid IL or missing references)
		//IL_014c: Unknown result type (might be due to invalid IL or missing references)
		//IL_015a: Unknown result type (might be due to invalid IL or missing references)
		//IL_017b: Unknown result type (might be due to invalid IL or missing references)
		//IL_0193: Unknown result type (might be due to invalid IL or missing references)
		//IL_01b4: Unknown result type (might be due to invalid IL or missing references)
		//IL_0211: Unknown result type (might be due to invalid IL or missing references)
		//IL_0213: Unknown result type (might be due to invalid IL or missing references)
		//IL_02a1: Unknown result type (might be due to invalid IL or missing references)
		//IL_02c9: Unknown result type (might be due to invalid IL or missing references)
		//IL_02d1: Unknown result type (might be due to invalid IL or missing references)
		//IL_033b: Unknown result type (might be due to invalid IL or missing references)
		//IL_03b1: Unknown result type (might be due to invalid IL or missing references)
		//IL_03b3: Unknown result type (might be due to invalid IL or missing references)
		//IL_0468: Unknown result type (might be due to invalid IL or missing references)
		//IL_0490: Unknown result type (might be due to invalid IL or missing references)
		//IL_0498: Unknown result type (might be due to invalid IL or missing references)
		//IL_04a6: Unknown result type (might be due to invalid IL or missing references)
		//IL_04c7: Unknown result type (might be due to invalid IL or missing references)
		//IL_04e8: Unknown result type (might be due to invalid IL or missing references)
		//IL_0509: Unknown result type (might be due to invalid IL or missing references)
		//IL_058f: Unknown result type (might be due to invalid IL or missing references)
		//IL_0591: Unknown result type (might be due to invalid IL or missing references)
		//IL_0646: Unknown result type (might be due to invalid IL or missing references)
		//IL_066e: Unknown result type (might be due to invalid IL or missing references)
		//IL_0676: Unknown result type (might be due to invalid IL or missing references)
		//IL_0684: Unknown result type (might be due to invalid IL or missing references)
		//IL_06a5: Unknown result type (might be due to invalid IL or missing references)
		//IL_06bd: Unknown result type (might be due to invalid IL or missing references)
		//IL_06de: Unknown result type (might be due to invalid IL or missing references)
		//IL_075b: Unknown result type (might be due to invalid IL or missing references)
		//IL_075d: Unknown result type (might be due to invalid IL or missing references)
		//IL_081f: Unknown result type (might be due to invalid IL or missing references)
		//IL_0849: Unknown result type (might be due to invalid IL or missing references)
		//IL_0851: Unknown result type (might be due to invalid IL or missing references)
		//IL_0886: Unknown result type (might be due to invalid IL or missing references)
		//IL_0899: Unknown result type (might be due to invalid IL or missing references)
		//IL_08c8: Unknown result type (might be due to invalid IL or missing references)
		//IL_08e0: Unknown result type (might be due to invalid IL or missing references)
		//IL_0909: Unknown result type (might be due to invalid IL or missing references)
		//IL_0a5b: Unknown result type (might be due to invalid IL or missing references)
		//IL_0a5d: Unknown result type (might be due to invalid IL or missing references)
		//IL_0a96: Unknown result type (might be due to invalid IL or missing references)
		//IL_0a9d: Unknown result type (might be due to invalid IL or missing references)
		//IL_0aaf: Unknown result type (might be due to invalid IL or missing references)
		//IL_0ab7: Unknown result type (might be due to invalid IL or missing references)
		//IL_0acc: Unknown result type (might be due to invalid IL or missing references)
		//IL_0ad4: Unknown result type (might be due to invalid IL or missing references)
		//IL_0ae9: Unknown result type (might be due to invalid IL or missing references)
		//IL_0bca: Unknown result type (might be due to invalid IL or missing references)
		//IL_0bcc: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c05: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c0c: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c1e: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c26: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c3b: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c43: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c58: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c91: Unknown result type (might be due to invalid IL or missing references)
		//IL_0c96: Unknown result type (might be due to invalid IL or missing references)
		//IL_0cba: Unknown result type (might be due to invalid IL or missing references)
		//IL_0d06: Unknown result type (might be due to invalid IL or missing references)
		//IL_0d21: Unknown result type (might be due to invalid IL or missing references)
		//IL_0d51: Unknown result type (might be due to invalid IL or missing references)
		//IL_0d57: Invalid comparison between Unknown and I4
		//IL_0d77: Unknown result type (might be due to invalid IL or missing references)
		//IL_0dab: Unknown result type (might be due to invalid IL or missing references)
		//IL_0e20: Unknown result type (might be due to invalid IL or missing references)
		//IL_0e25: Unknown result type (might be due to invalid IL or missing references)
		//IL_0e48: Unknown result type (might be due to invalid IL or missing references)
		//IL_0e8d: Unknown result type (might be due to invalid IL or missing references)
		//IL_0ea8: Unknown result type (might be due to invalid IL or missing references)
		//IL_0ed4: Unknown result type (might be due to invalid IL or missing references)
		//IL_0eda: Invalid comparison between Unknown and I4
		//IL_0efa: Unknown result type (might be due to invalid IL or missing references)
		//IL_0f2e: Unknown result type (might be due to invalid IL or missing references)
		if (RewardCrate.Count > 0)
		{
			Rect val = default(Rect);
			((Rect)(ref val))..ctor(0f, 0f, (float)CrateTexture.get_width(), (float)CrateTexture.get_height());
			RewardDisplay rewardDisplay = (RewardDisplay)RewardCrate[0];
			Rect val2 = val;
			float num2 = rewardDisplay.fDisplayTime / fRewardIconTime;
			float num3 = 2f - num2;
			((Rect)(ref val2)).set_x((float)(Screen.get_width() / 2 - 150 - 75));
			if (rewardDisplay.fDisplayTime > 0f)
			{
				((Rect)(ref val2)).set_y(Mathf.Abs(Mathf.Sin((float)Math.PI / 2f + (float)Math.PI * num3 * num3 * num3)) * num2 * num2 * (170f + num));
			}
			else
			{
				((Rect)(ref val2)).set_y(0f);
				white.a = Mathf.Min(1f, (1f + rewardDisplay.fDisplayTime / fRewardIconTime) * 2f);
			}
			GUI.set_color(white);
			((Rect)(ref val2)).set_y(((Rect)(ref val2)).get_y() + ((float)Screen.get_height() - (170f + num)));
			GUI.BeginGroup(val2);
			GUI.Label(val, CrateTexture);
			GUI.set_color(Color.get_black());
			GUI.Label(new Rect(1f, 111f, ((Rect)(ref val)).get_width(), 30f), rewardDisplay.title, MenuCenterBox3);
			GUI.set_color(Color.get_white());
			GUI.Label(new Rect(0f, 110f, ((Rect)(ref val)).get_width(), 30f), rewardDisplay.title, MenuCenterBox3);
			GUI.EndGroup();
		}
		else if (bFullInven)
		{
			Rect val3 = default(Rect);
			((Rect)(ref val3))..ctor(0f, 0f, (float)CrateTexture.get_width(), (float)CrateTexture.get_height());
			Rect val4 = val3;
			float num4 = fFullTimer / fRewardIconTime;
			float num5 = 2f - num4;
			((Rect)(ref val4)).set_x((float)(Screen.get_width() / 2 - 150 - 75));
			if (fFullTimer > 0f)
			{
				((Rect)(ref val4)).set_y(Mathf.Abs(Mathf.Sin((float)Math.PI / 2f + (float)Math.PI * num5 * num5 * num5)) * num4 * num4 * (170f + num));
			}
			else
			{
				((Rect)(ref val4)).set_y(0f);
			}
			GUI.set_color(Color.get_white());
			((Rect)(ref val4)).set_y(((Rect)(ref val4)).get_y() + ((float)Screen.get_height() - (170f + num)));
			GUI.BeginGroup(val4);
			GUI.Label(val3, CrateTexture);
			GUI.Label(new Rect(((Rect)(ref val3)).get_width() / 2f - (float)(FullInvenTex.get_width() / 2) - 5f, ((Rect)(ref val3)).get_height() / 2f - (float)(FullInvenTex.get_height() / 2) + 15f, (float)FullInvenTex.get_width(), (float)FullInvenTex.get_height()), FullInvenTex);
			GUI.EndGroup();
		}
		if (RewardPotion.Count > 0)
		{
			Rect val5 = default(Rect);
			((Rect)(ref val5))..ctor(0f, 0f, (float)PotionTexture.get_width(), (float)PotionTexture.get_height());
			RewardDisplay rewardDisplay2 = (RewardDisplay)RewardPotion[0];
			Rect val6 = val5;
			float num6 = rewardDisplay2.fDisplayTime / fRewardIconTime;
			float num7 = 2f - num6;
			((Rect)(ref val6)).set_x((float)(Screen.get_width() / 2 - 75));
			if (rewardDisplay2.fDisplayTime > 0f)
			{
				((Rect)(ref val6)).set_y(Mathf.Abs(Mathf.Sin((float)Math.PI / 2f + (float)Math.PI * num7 * num7 * num7)) * num6 * num6 * (170f + num));
			}
			else
			{
				((Rect)(ref val6)).set_y(0f);
				white.a = Mathf.Min(1f, (1f + rewardDisplay2.fDisplayTime / fRewardIconTime) * 2f);
			}
			GUI.set_color(white);
			((Rect)(ref val6)).set_y(((Rect)(ref val6)).get_y() + ((float)Screen.get_height() - (170f + num)));
			GUI.BeginGroup(val6);
			GUI.Label(val5, PotionTexture);
			GUI.set_color(Color.get_black());
			GUI.Label(new Rect(1f, 111f, ((Rect)(ref val5)).get_width(), 30f), rewardDisplay2.title, GUI.get_skin().GetStyle("centerbox3"));
			GUI.set_color(Color.get_white());
			GUI.Label(new Rect(0f, 110f, ((Rect)(ref val5)).get_width(), 30f), rewardDisplay2.title, GUI.get_skin().GetStyle("centerbox3"));
			GUI.EndGroup();
		}
		if (RewardBoost.Count > 0)
		{
			Rect val7 = default(Rect);
			((Rect)(ref val7))..ctor(0f, 0f, (float)BoostTexture.get_width(), (float)BoostTexture.get_height());
			RewardDisplay rewardDisplay3 = (RewardDisplay)RewardBoost[0];
			Rect val8 = val7;
			float num8 = rewardDisplay3.fDisplayTime / fRewardIconTime;
			float num9 = 2f - num8;
			((Rect)(ref val8)).set_x((float)(Screen.get_width() / 2 + 75));
			if (rewardDisplay3.fDisplayTime > 0f)
			{
				((Rect)(ref val8)).set_y(Mathf.Abs(Mathf.Sin((float)Math.PI / 2f + (float)Math.PI * num9 * num9 * num9)) * num8 * num8 * (170f + num));
			}
			else
			{
				((Rect)(ref val8)).set_y(0f);
				white.a = Mathf.Min(1f, (1f + rewardDisplay3.fDisplayTime / fRewardIconTime) * 2f);
			}
			GUI.set_color(white);
			((Rect)(ref val8)).set_y(((Rect)(ref val8)).get_y() + ((float)Screen.get_height() - (170f + num)));
			GUI.BeginGroup(val8);
			GUI.Label(val7, BoostTexture);
			GUI.set_color(Color.get_black());
			GUI.Label(new Rect(1f, 111f, ((Rect)(ref val7)).get_width(), 30f), rewardDisplay3.title, MenuCenterBox3);
			GUI.set_color(Color.get_white());
			GUI.Label(new Rect(0f, 110f, ((Rect)(ref val7)).get_width(), 30f), rewardDisplay3.title, MenuCenterBox3);
			GUI.EndGroup();
		}
		if (ShinyBuffIcon.Count > 0)
		{
			Rect val9 = default(Rect);
			((Rect)(ref val9))..ctor(0f, 0f, (float)ShinyTexture.get_width(), (float)ShinyTexture.get_height());
			RewardDisplay rewardDisplay4 = (RewardDisplay)ShinyBuffIcon[0];
			Rect val10 = val9;
			float num10 = rewardDisplay4.fDisplayTime / fRewardIconTime;
			float num11 = 2f - num10;
			((Rect)(ref val10)).set_x((float)(Screen.get_width() / 2 - ShinyTexture.get_width() / 2));
			if (rewardDisplay4.fDisplayTime > 0f)
			{
				((Rect)(ref val10)).set_y(Mathf.Abs(Mathf.Sin((float)Math.PI / 2f + (float)Math.PI * num11 * num11 * num11)) * num10 * num10 * (((Rect)(ref val10)).get_height() + num));
			}
			else
			{
				((Rect)(ref val10)).set_y(0f);
				white.a = Mathf.Min(1f, (1f + rewardDisplay4.fDisplayTime / fRewardIconTime) * 2f);
			}
			GUI.set_color(white);
			((Rect)(ref val10)).set_y(((Rect)(ref val10)).get_y() + ((float)Screen.get_height() - (((Rect)(ref val10)).get_height() + num)));
			GUI.BeginGroup(val10);
			GUI.Label(val9, ShinyTexture);
			if (Object.op_Implicit((Object)(object)rewardDisplay4.Icon))
			{
				GUI.Label(new Rect(71f, 72f, 22f, 22f), rewardDisplay4.Icon);
			}
			GUI.set_color(Color.get_black());
			GUI.Label(new Rect(1f, ((Rect)(ref val10)).get_height() - 25f + 1f, ((Rect)(ref val9)).get_width(), 25f), rewardDisplay4.title, MenuCenterBox3);
			GUI.set_color(Color.get_white());
			GUI.Label(new Rect(0f, ((Rect)(ref val10)).get_height() - 25f, ((Rect)(ref val9)).get_width(), 25f), rewardDisplay4.title, MenuCenterBox3);
			GUI.EndGroup();
		}
		if (QuestItemList.Count > 0)
		{
			RewardDisplay rewardDisplay5 = (RewardDisplay)QuestItemList[0];
			if (rewardDisplay5.iCount > 0)
			{
				Rect val11 = default(Rect);
				((Rect)(ref val11))..ctor(0f, 0f, (float)(rewardDisplay5.title.Length * 16 + 20), 40f);
				Color val12 = default(Color);
				((Color)(ref val12))..ctor(0.6509804f, 0.6901961f, 1f);
				((Rect)(ref val11)).set_x((float)(Screen.get_width() / 2 - rewardDisplay5.title.Length * 8));
				((Rect)(ref val11)).set_y((float)Screen.get_height() * 0.25f);
				if (rewardDisplay5.fDisplayTime < 1f)
				{
					float num12 = 1f - rewardDisplay5.fDisplayTime;
					val12.a = rewardDisplay5.fDisplayTime;
					((Rect)(ref val11)).set_x(((Rect)(ref val11)).get_x() + 100f * num12);
					((Rect)(ref val11)).set_y(((Rect)(ref val11)).get_y() + (280f - (float)Screen.get_height() * 0.25f) * num12);
				}
				Rect val13 = val11;
				((Rect)(ref val13)).set_x(((Rect)(ref val13)).get_x() + 1f);
				((Rect)(ref val13)).set_y(((Rect)(ref val13)).get_y() + 1f);
				GUI.set_color(new Color(0f, 0f, 0f, val12.a));
				FFGUIUtility.UnscaleAroundPivot((ScreenPivot)6);
				FFGUIUtility.ScaleAroundRect(val11);
				GUI.Label(val13, rewardDisplay5.title, MenuBigFont16);
				GUI.set_color(val12);
				GUI.Label(val11, rewardDisplay5.title, MenuBigFont16);
				FFGUIUtility.UnscaleAroundRect(val11);
				FFGUIUtility.ScaleAroundPivot((ScreenPivot)6);
			}
			else
			{
				Rect val14 = default(Rect);
				((Rect)(ref val14))..ctor(0f, 0f, (float)(rewardDisplay5.title.Length * 16 + 20), 40f);
				Color val15 = default(Color);
				((Color)(ref val15))..ctor(78f / 85f, 0.20392157f, 12f / 85f);
				((Rect)(ref val14)).set_x((float)Screen.get_width() * 0.5f - (float)(rewardDisplay5.title.Length * 8));
				((Rect)(ref val14)).set_y((float)Screen.get_height() * 0.25f);
				if (rewardDisplay5.fDisplayTime < 1f)
				{
					float num13 = 1f - rewardDisplay5.fDisplayTime;
					val15.a = rewardDisplay5.fDisplayTime;
					((Rect)(ref val14)).set_y(((Rect)(ref val14)).get_y() + 150f * num13);
				}
				Rect val16 = val14;
				((Rect)(ref val16)).set_x(((Rect)(ref val16)).get_x() + 1f);
				((Rect)(ref val16)).set_y(((Rect)(ref val16)).get_y() + 1f);
				GUI.set_color(new Color(0f, 0f, 0f, val15.a));
				FFGUIUtility.UnscaleAroundPivot((ScreenPivot)6);
				FFGUIUtility.ScaleAroundRect(val14);
				GUI.Label(val16, rewardDisplay5.title, MenuBigFont16);
				GUI.set_color(val15);
				GUI.Label(val14, rewardDisplay5.title, MenuBigFont16);
				FFGUIUtility.UnscaleAroundRect(val14);
				FFGUIUtility.ScaleAroundPivot((ScreenPivot)6);
			}
		}
		Rect val17 = default(Rect);
		if (iCurFM != iTargetFM || fFM_Timer > 0f)
		{
			Color white2 = Color.get_white();
			if (fFM_Timer < 1f)
			{
				white2.a = fFM_Timer;
			}
			GUI.set_color(white2);
			((Rect)(ref val17))..ctor(0f, 0f, 172f, 63f);
			((Rect)(ref val17)).set_x((float)(Screen.get_width() / 2 - 172));
			((Rect)(ref val17)).set_y((float)(Screen.get_height() - 63) - num);
			GUI.BeginGroup(val17);
			GUI.Label(new Rect(0f, 0f, 172f, 63f), FMTexture);
			int num14 = iCurFM;
			int num15 = 100000000;
			for (int i = 0; i < 9; i++)
			{
				int num16 = num14 / num15;
				if ((int)localized.local == 0)
				{
					GUI.Label(new Rect((float)(6 + 12 * i), 31f, 10f, 20f), num16.ToString(), MenuCenterBox3);
				}
				else
				{
					GUI.Label(new Rect((float)(5 + 12 * i), 31f, 10f, 20f), num16.ToString(), MenuCenterBox3);
				}
				num14 -= num16 * num15;
				num15 /= 10;
			}
			GUI.EndGroup();
		}
		if (iCurTaros == iTargetTaros && !(fTaros_Timer > 0f))
		{
			return;
		}
		Color white3 = Color.get_white();
		if (fFM_Timer < 1f)
		{
			white3.a = fTaros_Timer;
		}
		GUI.set_color(white3);
		((Rect)(ref val17))..ctor(0f, 0f, 170f, 64f);
		((Rect)(ref val17)).set_x((float)(Screen.get_width() / 2));
		((Rect)(ref val17)).set_y((float)(Screen.get_height() - 64) - num);
		GUI.BeginGroup(val17);
		GUI.Label(new Rect(0f, 0f, 170f, 64f), TarosTexture);
		int num17 = iCurTaros;
		int num18 = 100000000;
		for (int j = 0; j < 9; j++)
		{
			int num19 = num17 / num18;
			if ((int)localized.local == 0)
			{
				GUI.Label(new Rect((float)(6 + 12 * j), 31f, 10f, 20f), num19.ToString(), MenuCenterBox3);
			}
			else
			{
				GUI.Label(new Rect((float)(5 + 12 * j), 31f, 10f, 20f), num19.ToString(), MenuCenterBox3);
			}
			num17 -= num19 * num18;
			num18 /= 10;
		}
		GUI.EndGroup();
	}
}
