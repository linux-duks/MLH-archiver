#!/usr/bin/env perl
# Generate synthetic public-inbox test data matching NNTP test fixtures
use strict;
use warnings;
use v5.10.1;
use File::Path qw(make_path);
use File::Spec;
use Cwd qw(abs_path);

# Load required modules
eval {
    require PublicInbox::TestCommon;
    require PublicInbox::Eml;
    require PublicInbox::InboxWritable;
    require PublicInbox::Import;
    require YAML::XS;
    1;
} or do {
    die "Failed to load required modules: $@\n";
};

my $out_dir = shift @ARGV || '/test-data';
make_path($out_dir) unless -d $out_dir;

# Parse NNTP fixture YAML
my $yaml_file = 'fixtures/db.yml';
my $data = YAML::XS::LoadFile($yaml_file);

# Group messages by group name
my %groups;
foreach my $msg (@{$data->{messages}}) {
    my $group = $msg->{group};
    push @{$groups{$group}}, $msg;
}

# Create email objects from fixture data
sub create_eml_from_fixture {
    my ($msg) = @_;
    
    # Combine head and body with blank line separator
    my $raw = $msg->{head} . "\n" . $msg->{body};
    
    # Parse timestamp (ISO format in fixture)
    my $ts = $msg->{ts};
    
    return PublicInbox::Eml->new($raw);
}

# Create V2 inbox for each group
foreach my $group_name (sort keys %groups) {
    say "Creating V2 inbox for $group_name";
    
    my $group_dir = File::Spec->catdir($out_dir, "v2_$group_name");
    
    my $ibx = PublicInbox::TestCommon::create_inbox(
        "v2_$group_name",
        version => 2,
        tmpdir => $group_dir,
        sub {
            my ($importer, $ibx) = @_;
            
            foreach my $msg (@{$groups{$group_name}}) {
                my $eml = create_eml_from_fixture($msg);
                $importer->add($eml);
            }
        }
    );
    
    say "  Created at $group_dir";
    
    # Also create a V1 version for comparison
    my $v1_dir = File::Spec->catdir($out_dir, "v1_$group_name");
    my $ibx_v1 = PublicInbox::TestCommon::create_inbox(
        "v1_$group_name",
        version => 1,
        tmpdir => $v1_dir,
        sub {
            my ($importer, $ibx) = @_;
            
            foreach my $msg (@{$groups{$group_name}}) {
                my $eml = create_eml_from_fixture($msg);
                $importer->add($eml);
            }
        }
    );
    
    say "  Created V1 at $v1_dir";
}

# Create a combined V2 inbox with all emails (simulating all.git with alternates)
say "Creating combined V2 inbox with all emails";
my $combined_dir = File::Spec->catdir($out_dir, "v2_combined");
my $ibx_combined = PublicInbox::TestCommon::create_inbox(
    "v2_combined",
    version => 2,
    tmpdir => $combined_dir,
    sub {
        my ($importer, $ibx) = @_;
        
        foreach my $group_name (sort keys %groups) {
            foreach my $msg (@{$groups{$group_name}}) {
                my $eml = create_eml_from_fixture($msg);
                $importer->add($eml);
            }
        }
    }
);

say "  Created combined at $combined_dir";

# Create an empty inbox for testing
say "Creating empty V2 inbox";
my $empty_dir = File::Spec->catdir($out_dir, "v2_empty");
my $ibx_empty = PublicInbox::TestCommon::create_inbox(
    "v2_empty",
    version => 2,
    tmpdir => $empty_dir,
    sub {
        # No emails added
    }
);

say "  Created empty at $empty_dir";

# Create a multi-epoch V2 inbox with 3 epochs (12 emails total)
say "Creating multi-epoch V2 inbox with 3 epochs";
my $multi_epoch_dir = File::Spec->catdir($out_dir, "v2_multi_epoch.list");
my $ibx_multi_epoch = PublicInbox::TestCommon::create_inbox(
    "v2_multi_epoch.list",
    version => 2,
    tmpdir => $multi_epoch_dir,
    rotate_bytes => 1000,  # Very low threshold to force epoch rotation
    sub {
        my ($importer, $ibx) = @_;
        
        # Create 12 emails with distinct content for testing
        for my $i (1..12) {
            my $eml = PublicInbox::Eml->new(
                "From: tester\@example.org\n" .
                "Subject: Multi-epoch test email $i\n" .
                "Message-ID: <multi-$i\@example.org>\n" .
                "Date: " . scalar(localtime) . "\n" .
                "\n" .
                "This is email $i from the multi-epoch test inbox.\n" .
                "Epoch: " . (($i-1)/4) . " (0-based)\n"
            );
            $importer->add($eml);
        }
    }
);

say "  Created multi-epoch at $multi_epoch_dir";

# Create an all.git alternates-based V2 inbox
say "Creating all.git alternates-based V2 inbox";
my $alternates_dir = File::Spec->catdir($out_dir, "v2_all_git_alternates");
my $ibx_alternates = PublicInbox::TestCommon::create_inbox(
    "v2_all_git_alternates",
    version => 2,
    tmpdir => $alternates_dir,
    # Create a standard inbox first, then we'll set up alternates manually
    sub {
        my ($importer, $ibx) = @_;
        
        # Create a few emails to populate the initial structure
        for my $i (1..5) {
            my $eml = PublicInbox::Eml->new(
                "From: tester\@example.org\n" .
                "Subject: Alternates test email $i\n" .
                "Message-ID: <alternate-$i\@example.org>\n" .
                "Date: " . scalar(localtime) . "\n" .
                "\n" .
                "This is an alternate test email $i.\n"
            );
            $importer->add($eml);
        }
    }
);

say "  Created alternates-based at $alternates_dir";

say "\nTest data generation complete in $out_dir";
say "Inboxes created:";
foreach my $group_name (sort keys %groups) {
    say "  - v2_$group_name (V2 format)";
    say "  - v1_$group_name (V1 format)";
}
say "  - v2_combined (combined V2)";
say "  - v2_empty (empty V2)";
say "  - v2_multi_epoch.list (multi-epoch V2)";
say "  - v2_all_git_alternates (all.git alternates V2)";