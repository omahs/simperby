use super::*;

type Error = super::Error;

impl fmt::Debug for RawRepositoryImplInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

pub(crate) struct RawRepositoryImplInner {
    repo: Repository,
}

/// TODO: Error handling and its messages
impl RawRepositoryImplInner {
    pub(crate) fn init(
        directory: &str,
        init_commit_message: &str,
        init_commit_branch: &Branch,
    ) -> Result<Self, Error>
    where
        Self: Sized,
    {
        match Repository::open(directory) {
            Ok(_repo) => Err(Error::InvalidRepository(
                "there is an already existing repository".to_string(),
            )),
            Err(_e) => {
                let mut opts = RepositoryInitOptions::new();
                opts.initial_head(init_commit_branch.as_str());
                let repo = Repository::init_opts(directory, &opts)?;
                {
                    // Create initial empty commit
                    let mut config = repo.config()?;
                    config.set_str("user.name", "name")?; // TODO: user.name value
                    config.set_str("user.email", "email")?; // TODO: user.email value
                    let mut index = repo.index()?;
                    let id = index.write_tree()?;
                    let sig = repo.signature()?;
                    let tree = repo.find_tree(id)?;

                    let _oid =
                        repo.commit(Some("HEAD"), &sig, &sig, init_commit_message, &tree, &[])?;
                }

                Ok(Self { repo })
            }
        }
    }

    pub(crate) fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = Repository::open(directory)?;

        Ok(Self { repo })
    }

    pub(crate) fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        let branches = self.repo.branches(Option::Some(BranchType::Local))?;

        branches
            .map(|branch| {
                let branch_name = branch?
                    .0
                    .name()?
                    .map(|name| name.to_string())
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;

                Ok(branch_name)
            })
            .collect::<Result<Vec<Branch>, Error>>()
    }

    pub(crate) fn create_branch(
        &self,
        branch_name: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;

        // TODO: Test if force true and verify new branch is created
        self.repo.branch(branch_name.as_str(), &commit, false)?;

        Ok(())
    }

    pub(crate) fn locate_branch(&self, branch: Branch) -> Result<CommitHash, Error> {
        let branch = self.repo.find_branch(&branch, BranchType::Local)?;
        let oid = branch
            .get()
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    pub(crate) fn get_branches(&self, commit_hash: CommitHash) -> Result<Vec<Branch>, Error> {
        let oid_target = git2::Oid::from_bytes(&commit_hash.hash)?;

        let branches = self.repo.branches(Option::Some(BranchType::Local))?;
        let branches = branches.into_iter().collect::<Result<Vec<_>, _>>()?;
        let branches = branches
            .into_iter()
            .map(|(branch, _)| {
                let oid = branch.get().target();
                match oid {
                    Some(oid) => Ok((branch, oid)),
                    None => Err(Error::Unknown("err".to_string())),
                }
            })
            .collect::<Result<Vec<(git2::Branch, Oid)>, Error>>()?;

        let branches = branches
            .into_iter()
            .filter(|(_, oid)| *oid == oid_target)
            .map(|(branch, _)| {
                branch
                    .name()?
                    .map(|name| name.to_string())
                    .ok_or_else(|| Error::Unknown("err".to_string()))
            })
            .collect::<Result<Vec<Branch>, Error>>()?;

        Ok(branches)
    }

    pub(crate) fn move_branch(
        &mut self,
        branch: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error> {
        let mut git2_branch = self.repo.find_branch(&branch, BranchType::Local)?;
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let reflog_msg = ""; // TODO: reflog_msg
        let reference = git2_branch.get_mut();
        let _set_branch = git2::Reference::set_target(reference, oid, reflog_msg)?;

        Ok(())
    }

    pub(crate) fn delete_branch(&mut self, branch: Branch) -> Result<(), Error> {
        let mut git2_branch = self.repo.find_branch(&branch, BranchType::Local)?;

        let current_branch = self
            .repo
            .head()?
            .shorthand()
            .ok_or_else(|| Error::Unknown("err".to_string()))?
            .to_string();

        if current_branch == branch {
            Err(Error::InvalidRepository(
                ("given branch is currently checkout branch").to_string(),
            ))
        } else {
            git2_branch.delete().map_err(Error::from)
        }
    }

    pub(crate) fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        let tag_array = self.repo.tag_names(None)?;

        let tag_list = tag_array
            .iter()
            .map(|tag| {
                let tag_name = tag
                    .ok_or_else(|| Error::Unknown("err".to_string()))?
                    .to_string();

                Ok(tag_name)
            })
            .collect::<Result<Vec<Tag>, Error>>();

        tag_list
    }

    pub(crate) fn create_tag(&mut self, tag: Tag, commit_hash: CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let object = self.repo.find_object(oid, Some(ObjectType::Commit))?;
        self.repo.tag_lightweight(tag.as_str(), &object, true)?;

        Ok(())
    }

    pub(crate) fn locate_tag(&self, tag: Tag) -> Result<CommitHash, Error> {
        let reference = self
            .repo
            .find_reference(&("refs/tags/".to_owned() + &tag))?;
        let object = reference.peel(ObjectType::Commit)?;
        let oid = object.id();
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        let commit_hash = CommitHash { hash };
        Ok(commit_hash)
    }

    pub(crate) fn get_tag(&self, commit_hash: CommitHash) -> Result<Vec<Tag>, Error> {
        let oid_target = Oid::from_bytes(&commit_hash.hash)?;

        let references = self.repo.references_glob("refs/tags/*")?;
        let references = references.into_iter().collect::<Result<Vec<_>, _>>()?;
        let references = references
            .into_iter()
            .map(|reference| {
                let oid = reference
                    .target()
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;

                Ok((reference, oid))
            })
            .collect::<Result<Vec<(git2::Reference, Oid)>, Error>>()?;

        let tags = references
            .into_iter()
            .filter(|(_, oid)| *oid == oid_target)
            .map(|(reference, _)| {
                let tag = reference
                    .shorthand()
                    .ok_or_else(|| Error::Unknown("err".to_string()))?
                    .to_string();

                Ok(tag)
            })
            .collect::<Result<Vec<Tag>, Error>>()?;

        Ok(tags)
    }

    pub(crate) fn remove_tag(&mut self, tag: Tag) -> Result<(), Error> {
        self.repo.tag_delete(tag.as_str()).map_err(Error::from)
    }

    pub(crate) fn create_commit(
        &mut self,
        commit_message: String,
        _diff: Option<String>,
    ) -> Result<CommitHash, Error> {
        let sig = self.repo.signature()?;
        let mut index = self.repo.index()?;
        let id = index.write_tree()?;
        let tree = self.repo.find_tree(id)?;
        let head = self.get_head()?;
        let parent_oid = git2::Oid::from_bytes(&head.hash)?;
        let parent_commit = self.repo.find_commit(parent_oid)?;

        let oid = self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            commit_message.as_str(),
            &tree,
            &[&parent_commit],
        )?;

        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    pub(crate) fn create_semantic_commit(
        &mut self,
        commit: SemanticCommit,
    ) -> Result<CommitHash, Error> {
        match commit.diff {
            Diff::None => {
                let sig = self.repo.signature()?;
                let mut index = self.repo.index()?;
                let id = index.write_tree()?;
                let tree = self.repo.find_tree(id)?;
                let commit_message = format!("{}{}{}", commit.title, "\n", commit.body); // TODO: Check "\n" divides commit message's head and body.
                let head = self.get_head()?;
                let parent_oid = git2::Oid::from_bytes(&head.hash)?;
                let parent_commit = self.repo.find_commit(parent_oid)?;

                let oid = self.repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    commit_message.as_str(),
                    &tree,
                    &[&parent_commit],
                )?;

                let hash = <[u8; 20]>::try_from(oid.as_bytes())
                    .map_err(|_| Error::Unknown("err".to_string()))?;

                Ok(CommitHash { hash })
            }
            Diff::Reserved(reserved_state) => {
                let genesis_info = serde_json::to_string(&reserved_state.genesis_info).unwrap();
                let consensus_leader_order =
                    serde_json::to_string(&reserved_state.consensus_leader_order).unwrap();
                let version = serde_json::to_string(&reserved_state.version).unwrap();

                // Create files of reserved state.
                let path = Path::new(self.repo.workdir().unwrap()).join("reserved");
                if !path.exists() {
                    fs::create_dir(path.clone()).unwrap();
                }
                fs::write(path.join(Path::new("genesis_info.json")), genesis_info).unwrap();
                fs::write(
                    path.join(Path::new("consensus_leader_order.json")),
                    consensus_leader_order,
                )
                .unwrap();
                fs::write(path.join(Path::new("version")), version).unwrap();

                let mut index = self.repo.index()?;
                index.add_path(Path::new("reserved/genesis_info.json"))?;
                index.add_path(Path::new("reserved/consensus_leader_order.json"))?;
                index.add_path(Path::new("reserved/version"))?;

                let path = path.join("members");
                if !path.exists() {
                    fs::create_dir(path.clone()).unwrap();
                }
                for member in reserved_state.members {
                    let file_name = format!("{}{}", member.name, ".json");
                    let member = serde_json::to_string(&member).unwrap();
                    fs::write(path.join(file_name.as_str()), member).unwrap();
                    index.add_path(&path.join(file_name.as_str()))?;
                }

                let sig = self.repo.signature()?;
                let id = index.write_tree()?;
                let tree = self.repo.find_tree(id)?;
                let commit_message = format!("{}{}{}", commit.title, "\n", commit.body); // TODO: Check "\n" divides commit message's head and body.
                let head = self.get_head()?;
                let parent_oid = git2::Oid::from_bytes(&head.hash)?;
                let parent_commit = self.repo.find_commit(parent_oid)?;

                let oid = self.repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    commit_message.as_str(),
                    &tree,
                    &[&parent_commit],
                )?;

                let hash = <[u8; 20]>::try_from(oid.as_bytes())
                    .map_err(|_| Error::Unknown("err".to_string()))?;

                Ok(CommitHash { hash })
            }
            Diff::General(_, _) => Err(Error::InvalidRepository(
                "diff is Diff::General()".to_string(),
            )),
            Diff::NonReserved(_) => Err(Error::InvalidRepository(
                "diff is Diff::NonReserved()".to_string(),
            )),
        }
    }

    pub(crate) fn read_semantic_commit(
        &self,
        commit_hash: CommitHash,
    ) -> Result<SemanticCommit, Error> {
        let oid = git2::Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = commit.parent(0)?.tree()?;

        // Create diff by verifying the commit made files or not.
        let diff = self
            .repo
            .diff_tree_to_tree(Some(&tree), Some(&parent_tree), None)?;
        let diff = if diff.deltas().len() == 0 {
            Diff::None
        } else {
            let reserved_state = self.read_reserved_state()?;
            Diff::Reserved(Box::new(reserved_state))
        };

        let commit_message = commit.message().unwrap().split('\n').collect::<Vec<_>>();
        let title = commit_message[0].to_string();
        let body = commit_message[1].to_string();
        let semantic_commit = SemanticCommit { title, body, diff };

        Ok(semantic_commit)
    }

    pub(crate) fn run_garbage_collection(&mut self) -> Result<(), Error> {
        todo!()
    }

    pub(crate) fn checkout_clean(&mut self) -> Result<(), Error> {
        todo!()
    }

    pub(crate) fn checkout(&mut self, branch: Branch) -> Result<(), Error> {
        let obj = self
            .repo
            .revparse_single(&("refs/heads/".to_owned() + &branch))?;
        self.repo.checkout_tree(&obj, None)?;
        self.repo.set_head(&("refs/heads/".to_owned() + &branch))?;

        Ok(())
    }

    pub(crate) fn checkout_detach(&mut self, commit_hash: CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        self.repo.set_head_detached(oid)?;

        Ok(())
    }

    pub(crate) fn get_head(&self) -> Result<CommitHash, Error> {
        let ref_head = self.repo.head()?;
        let oid = ref_head
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    pub(crate) fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        // Check if the repository is empty
        // TODO: Replace this with repo.empty()
        let _head = self
            .repo
            .head()
            .map_err(|_| Error::InvalidRepository("repository is empty".to_string()))?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()?;

        let initial_oid = if oids.len() == 1 { oids[0] } else { oids[1] };
        let hash = <[u8; 20]>::try_from(initial_oid.as_bytes())
            .map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    pub(crate) fn show_commit(&self, commit_hash: CommitHash) -> Result<String, Error> {
        let oid = git2::Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;

        if commit.parents().len() > 1 {
            //TODO: if parents > 1?
        }

        let mut emailopts = git2::EmailCreateOptions::new();
        let email = git2::Email::from_commit(&commit, &mut emailopts)?;
        let email = str::from_utf8(email.as_slice())
            .map_err(|_| Error::Unknown("err".to_string()))?
            .to_string();

        Ok(email)
    }

    pub(crate) fn list_ancestors(
        &self,
        commit_hash: CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(oid)?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

        // Compare max and ancestor's size
        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()?;
        let oids = oids[1..oids.len()].to_vec();

        let oids_ancestor = if let Some(num_max) = max {
            for &oid in oids.iter().take(num_max) {
                // TODO: Check first one should be commit_hash
                let commit = self.repo.find_commit(oid)?;
                let num_parents = commit.parents().len();

                if num_parents > 1 {
                    return Err(Error::InvalidRepository(format!(
                        "There exists a merge commit, {}",
                        oid
                    )));
                }
                // TODO: Should check current commit's parent == oids[next]
            }
            oids[0..num_max].to_vec()
        } else {
            // If max is None
            let mut i = 0;

            loop {
                // TODO: Check first one should be commit_hash
                let commit = self.repo.find_commit(oids[i])?;
                let num_parents = commit.parents().len();

                if num_parents > 1 {
                    return Err(Error::InvalidRepository(format!(
                        "There exists a merge commit, {}",
                        oid
                    )));
                }
                // TODO: Should check current commit's parent == oids[next]
                if num_parents == 0 {
                    break;
                }
                i += 1;
            }
            oids
        };

        let ancestors = oids_ancestor
            .iter()
            .map(|&oid| {
                let hash: [u8; 20] = oid
                    .as_bytes()
                    .try_into()
                    .map_err(|_| Error::Unknown("err".to_string()))?;
                Ok(CommitHash { hash })
            })
            .collect::<Result<Vec<CommitHash>, Error>>();

        ancestors
    }

    pub(crate) fn query_commit_path(
        &self,
        ancestor: CommitHash,
        descendant: CommitHash,
    ) -> Result<Vec<CommitHash>, Error> {
        if ancestor == descendant {
            return Err(Error::InvalidRepository(
                "ancestor and descendant are same".to_string(),
            ));
        }

        let merge_base = self.find_merge_base(ancestor, descendant)?;
        if merge_base != ancestor {
            return Err(Error::InvalidRepository(
                "ancestor is not the merge base of two commits".to_string(),
            ));
        }

        let descendant_oid = Oid::from_bytes(&descendant.hash)?;
        let ancestor_oid = Oid::from_bytes(&ancestor.hash)?;

        let mut revwalk = self.repo.revwalk()?;
        let range = format!("{}{}{}", ancestor_oid, "..", descendant_oid);
        revwalk.push_range(range.as_str())?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()?;

        let commits = oids
            .iter()
            .map(|&oid| {
                let hash: [u8; 20] = oid
                    .as_bytes()
                    .try_into()
                    .map_err(|_| Error::Unknown("err".to_string()))?;
                Ok(CommitHash { hash })
            })
            .collect::<Result<Vec<CommitHash>, Error>>()?;

        Ok(commits)
    }

    pub(crate) fn list_children(&self, _commit_hash: CommitHash) -> Result<Vec<CommitHash>, Error> {
        todo!()
    }

    pub(crate) fn find_merge_base(
        &self,
        commit_hash1: CommitHash,
        commit_hash2: CommitHash,
    ) -> Result<CommitHash, Error> {
        let oid1 = Oid::from_bytes(&commit_hash1.hash)?;
        let oid2 = Oid::from_bytes(&commit_hash2.hash)?;

        let oid_merge = self.repo.merge_base(oid1, oid2)?;
        let commit_hash_merge: [u8; 20] = oid_merge
            .as_bytes()
            .try_into()
            .map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash {
            hash: commit_hash_merge,
        })
    }

    pub(crate) fn read_reserved_state(&self) -> Result<ReservedState, Error> {
        let path = self.repo.workdir().unwrap().to_str().unwrap();
        let genesis_info =
            fs::read_to_string(format!("{}{}", path, "reserved/genesis_info.json")).unwrap();
        let genesis_info: GenesisInfo = serde_json::from_str(genesis_info.as_str()).unwrap();

        let mut members: Vec<Member> = vec![];
        let members_directory = fs::read_dir(format!("{}{}", path, "reserved/members")).unwrap();
        for file in members_directory {
            let path = file.unwrap().path();
            let path = path.to_str().unwrap();
            let member = fs::read_to_string(path).unwrap();
            let member: Member = serde_json::from_str(member.as_str()).unwrap();
            members.push(member);
        }

        let consensus_leader_order = fs::read_to_string(format!(
            "{}{}",
            path, "reserved/consensus_leader_order.json"
        ))
        .unwrap();
        let consensus_leader_order: Vec<usize> =
            serde_json::from_str(consensus_leader_order.as_str()).unwrap();

        let version = fs::read_to_string(format!("{}{}", path, "reserved/version")).unwrap();

        let reserved_state = ReservedState {
            genesis_info,
            members,
            consensus_leader_order,
            version,
        };

        Ok(reserved_state)
    }

    pub(crate) fn add_remote(
        &mut self,
        remote_name: String,
        remote_url: String,
    ) -> Result<(), Error> {
        self.repo
            .remote(remote_name.as_str(), remote_url.as_str())?;

        Ok(())
    }

    pub(crate) fn remove_remote(&mut self, remote_name: String) -> Result<(), Error> {
        self.repo.remote_delete(remote_name.as_str())?;

        Ok(())
    }

    pub(crate) fn fetch_all(&mut self) -> Result<(), Error> {
        let remote_list = self.repo.remotes()?;
        let remote_list = remote_list
            .iter()
            .map(|remote| {
                let remote_name =
                    remote.ok_or_else(|| Error::Unknown("unable to get remote".to_string()))?;

                Ok(remote_name)
            })
            .collect::<Result<Vec<&str>, Error>>()?;

        for name in remote_list {
            let mut remote = self.repo.find_remote(name)?;
            remote.fetch(&[] as &[&str], None, None)?;
        }

        Ok(())
    }

    pub(crate) fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        let remote_array = self.repo.remotes()?;

        let remote_list = remote_array
            .iter()
            .map(|remote| {
                let remote_name = remote
                    .ok_or_else(|| Error::Unknown("unable to get remote".to_string()))?
                    .to_string();

                Ok(remote_name)
            })
            .collect::<Result<Vec<String>, Error>>()?;

        let remote_list = remote_list
            .iter()
            .map(|name| {
                let remote = self.repo.find_remote(name.clone().as_str())?;

                let url = remote
                    .url()
                    .ok_or_else(|| Error::Unknown("unable to get valid url".to_string()))?;

                Ok((name.clone(), url.to_string()))
            })
            .collect::<Result<Vec<(String, String)>, Error>>()?;

        Ok(remote_list)
    }

    pub(crate) fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error> {
        let branches = self.repo.branches(Some(git2::BranchType::Remote))?;
        let branches = branches
            .map(|branch| {
                let branch_name = branch?
                    .0
                    .name()?
                    .map(|name| name.to_string())
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;

                Ok(branch_name)
            })
            .collect::<Result<Vec<Branch>, Error>>()?;

        let branches = branches
            .iter()
            .map(|branch| {
                let names: Vec<&str> = branch.split('/').collect();
                let remote_name = names[0];
                let branch_name = names[1];
                let branch = self.repo.find_branch(branch, BranchType::Remote)?;

                let oid = branch
                    .get()
                    .target()
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;
                let hash = <[u8; 20]>::try_from(oid.as_bytes())
                    .map_err(|_| Error::Unknown("err".to_string()))?;
                let commit_hash = CommitHash { hash };

                Ok((
                    remote_name.to_string(),
                    branch_name.to_string(),
                    commit_hash,
                ))
            })
            .collect::<Result<Vec<(String, String, CommitHash)>, Error>>()?;

        Ok(branches)
    }

    pub(crate) fn locate_remote_tracking_branch(
        &self,
        remote_name: String,
        branch_name: String,
    ) -> Result<CommitHash, Error> {
        let name = format!("{}/{}", remote_name, branch_name);
        let branch = self.repo.find_branch(name.as_str(), BranchType::Remote)?;

        let oid = branch
            .get()
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        let commit_hash = CommitHash { hash };

        Ok(commit_hash)
    }
}
